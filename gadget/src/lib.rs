//! Webb Relayer Gadget
//!
//! Integrates the Webb Relayer into the Substrate Node.
use eth2_pallet_init::{init_pallet, substrate_pallet_client::EthClientPallet};
use eth2_to_substrate_relay::eth2substrate_relay::Eth2SubstrateRelay;
use std::path::PathBuf;
use subxt::OnlineClient;
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

	let api =
		OnlineClient::<subxt::PolkadotConfig>::from_url(lc_relay_config.substrate_endpoint.clone())
			.await
			.expect("failed to connect to substrate node");

	let mut eth_pallet =
		EthClientPallet::new(api, lc_relay_config.ethereum_network.as_typed_chain_id());

	let mut relay = Eth2SubstrateRelay::init(&lc_relay_config, Box::new(eth_pallet.clone())).await;

	tracing::info!(target: "relay", "=== Initializing relay ===");

	if let Ok(is_initialized) = eth_pallet
		.is_initialized(init_pallet::get_typed_chain_id(&lc_init_config))
		.await
	{
		if !is_initialized {
			match init_pallet::init_pallet(&lc_init_config, &mut eth_pallet).await {
				Ok(_) => tracing::info!(target: "relay", "=== Pallet initialized ==="),
				Err(e) =>
					tracing::error!(target: "relay", "=== Failed to initialize pallet: {:?} ===", e),
			};
		}
	}

	tracing::info!(target: "relay", "=== Relay initialized ===");
	relay.run(None).await;
}

/// Loads the configuration for the light client
fn loads_light_client_relayer_config(
	config_path: &PathBuf,
) -> anyhow::Result<eth2_to_substrate_relay::config::Config> {
	Ok(eth2_to_substrate_relay::config::Config::load_from_toml(config_path.clone()))
}

/// Loads the configuration for the light client
fn loads_light_client_pallet_init_config(
	config_path: &PathBuf,
) -> anyhow::Result<eth2_pallet_init::config::Config> {
	Ok(eth2_pallet_init::config::Config::load_from_toml(config_path.clone()))
}
