use eth2_pallet_init::config::Config as InitConfig;
use lc_relay_config::RelayConfig;
use subxt::OnlineClient;
use tokio::sync::broadcast;

/// LightClientRelayerContext contains Relayer's configuration and shutdown signal.
#[derive(Clone)]
pub struct LightClientRelayerContext {
	pub lc_relay_config: RelayConfig,
	pub lc_init_config: InitConfig,
	/// Broadcasts a shutdown signal to all active connections.
	///
	/// The initial `shutdown` trigger is provided by the `run` caller. The
	/// server is responsible for gracefully shutting down active connections.
	/// When a connection task is spawned, it is passed a broadcast receiver
	/// handle. When a graceful shutdown is initiated, a `()` value is sent via
	/// the broadcast::Sender. Each active connection receives it, reaches a
	/// safe terminal state, and completes the task.
	notify_shutdown: broadcast::Sender<()>,
}

impl LightClientRelayerContext {
	pub fn new(lc_relay_config: RelayConfig, lc_init_config: InitConfig) -> Self {
		let (notify_shutdown, _) = broadcast::channel(2);
		Self { lc_relay_config, lc_init_config, notify_shutdown }
	}
	pub async fn substrate_provider(self) -> anyhow::Result<OnlineClient<subxt::PolkadotConfig>> {
		let maybe_client = OnlineClient::<subxt::PolkadotConfig>::from_url(
			self.lc_relay_config.substrate_endpoint.clone(),
		)
		.await;
		let client = match maybe_client {
			Ok(client) => client,
			Err(err) => return Err(err.into()),
		};
		Ok(client)
	}
	/// Returns a broadcast receiver handle for the shutdown signal.
	pub fn shutdown_signal(&self) -> Shutdown {
		Shutdown::new(self.notify_shutdown.subscribe())
	}
	/// Sends a shutdown signal to all subscribed tasks/connections.
	pub fn shutdown(&self) {
		let _ = self.notify_shutdown.send(());
	}
}

/// Listens for the server shutdown signal.
///
/// Shutdown is signalled using a `broadcast::Receiver`. Only a single value is
/// ever sent. Once a value has been sent via the broadcast channel, the server
/// should shutdown.
///
/// The `Shutdown` struct listens for the signal and tracks that the signal has
/// been received. Callers may query for whether the shutdown signal has been
/// received or not.
#[derive(Debug)]
pub struct Shutdown {
	/// `true` if the shutdown signal has been received
	shutdown: bool,

	/// The receive half of the channel used to listen for shutdown.
	notify: broadcast::Receiver<()>,
}

impl Shutdown {
	/// Create a new `Shutdown` backed by the given `broadcast::Receiver`.
	pub fn new(notify: broadcast::Receiver<()>) -> Shutdown {
		Shutdown { shutdown: false, notify }
	}

	/// Receive the shutdown notice, waiting if necessary.
	pub async fn recv(&mut self) {
		// If the shutdown signal has already been received, then return
		// immediately.
		if self.shutdown {
			return;
		}

		// Cannot receive a "lag error" as only one value is ever sent.
		let _ = self.notify.recv().await;

		// Remember that the signal has been received.
		self.shutdown = true;
	}
}
