//! Webb Relayer Gadget
//!
//! Integrates the Webb Relayer into the Substrate Node.
use eth2_pallet_init::{init_pallet, substrate_pallet_client::EthClientPallet};
use eth2_to_substrate_relay::eth2substrate_relay::Eth2SubstrateRelay;
use lc_relay_config::RelayConfig;
use lc_relayer_context::LightClientRelayerContext;
use std::{path::PathBuf, sync::Arc};
use tokio::signal::unix;
use webb_proposals::TypedChainId;
pub mod errors;

/// Webb Relayer gadget initialization parameters.
pub struct Eth2LightClientParams {
	/// Light client relayer configuration path
	pub lc_relay_config_path: Option<PathBuf>,
	/// Light client init pallet configuration path
	pub lc_init_config_path: Option<PathBuf>,
	/// Eth2 Chain Identifier
	pub eth2_chain_id: TypedChainId,
}

pub async fn ignite_lc_relayer(ctx: LightClientRelayerContext) -> anyhow::Result<()> {
	let backoff = backoff::ExponentialBackoff { max_elapsed_time: None, ..Default::default() };

	let task = || async {
		let maybe_client = ctx.clone().substrate_provider().await;
		let api_client = match maybe_client {
			Ok(client) => client,
			Err(err) => {
				tracing::error!("Failed to connect with substrate client, retrying...!");
				return Err(backoff::Error::transient(err))
			},
		};
		let api_client = Arc::new(api_client);
		let mut eth_pallet = EthClientPallet::new(
			api_client,
			ctx.lc_relay_config.ethereum_network.as_typed_chain_id(),
		);
		let mut relay =
			Eth2SubstrateRelay::init(&ctx.lc_relay_config, Box::new(eth_pallet.clone())).await;
		tracing::info!(target: "relay", "=== Initializing relay ===");

		if let Ok(is_initialized) = eth_pallet
			.is_initialized(init_pallet::get_typed_chain_id(&ctx.lc_init_config))
			.await
		{
			if !is_initialized {
				match init_pallet::init_pallet(&ctx.lc_init_config, &mut eth_pallet).await {
					Ok(_) => tracing::info!(target: "relay", "=== Pallet initialized ==="),
					Err(e) => {
						tracing::error!(target: "relay", "=== Failed to initialize pallet: {:?} ===", e);
						return Err(backoff::Error::permanent(e))
					},
				};
			}
		}
		tracing::info!(target: "relay", "=== Relay initialized ===");
		relay.run(None).await.map_err(backoff::Error::transient)?;
		Ok(())
	};
	backoff::future::retry(backoff, task).await?;
	Ok(())
}

pub async fn start_gadget(relayer_params: Eth2LightClientParams) {
	// Light Client Relayer
	let lc_relay_config = match relayer_params.lc_relay_config_path.as_ref() {
		Some(p) =>
			loads_light_client_relayer_config(p).expect("failed to load light client config"),
		None => {
			tracing::error!(
				target: "light-client-gadget",
				"Error: Not Starting ETH2 Light Client Relayer Gadget. No Config Directory Specified"
			);
			return
		},
	};

	let lc_init_config = match relayer_params.lc_init_config_path.as_ref() {
		Some(p) => loads_light_client_pallet_init_config(p)
			.expect("failed to load light client init pallet config"),
		None => {
			tracing::error!(
				target: "light-client-gadget",
				"Error: Not Starting ETH2 Light Client Relayer Gadget. No Config Directory Specified"
			);
			return
		},
	};
	let ctx = LightClientRelayerContext::new(lc_relay_config, lc_init_config);
	ignite_lc_relayer(ctx.clone())
		.await
		.expect("failed to start light client relayer");
	// watch for signals
	let mut ctrlc_signal =
		unix::signal(unix::SignalKind::interrupt()).expect("failed to register ctrlc handler");
	let mut termination_signal = unix::signal(unix::SignalKind::terminate())
		.expect("failed to register termination handler");
	let mut quit_signal =
		unix::signal(unix::SignalKind::quit()).expect("failed to register quit handler");
	let shutdown = || {
		tracing::warn!("Shutting down...");
		// shut down storage fetching
		// send shutdown signal to all of the application.
		ctx.shutdown();
		std::thread::sleep(std::time::Duration::from_millis(300));
		tracing::info!("Clean Exit ..");
	};
	tokio::select! {
		_ = ctrlc_signal.recv() => {
			tracing::warn!("Interrupted (Ctrl+C) ...");
			shutdown();
		},
		_ = termination_signal.recv() => {
			tracing::warn!("Got Terminate signal ...");
			shutdown();
		},
		_ = quit_signal.recv() => {
			tracing::warn!("Quitting ...");
			shutdown();
		},
	}
}

/// Loads the configuration for the light client
fn loads_light_client_relayer_config(config_path: &PathBuf) -> anyhow::Result<RelayConfig> {
	Ok(RelayConfig::load_from_toml(config_path.clone()))
}

/// Loads the configuration for the light client
fn loads_light_client_pallet_init_config(
	config_path: &PathBuf,
) -> anyhow::Result<eth2_pallet_init::config::Config> {
	Ok(eth2_pallet_init::config::Config::load_from_toml(config_path.clone()))
}
