//! Webb Relayer Gadget
//!
//! Integrates the Webb Relayer into the Substrate Node.
#![allow(dead_code)]
use dkg_runtime_primitives::crypto;
use eth2_pallet_init::{init_pallet, substrate_pallet_client::EthClientPallet};
use eth2_to_substrate_relay::eth2substrate_relay::Eth2SubstrateRelay;
use ethereum_types::Secret;
use sc_keystore::LocalKeystore;
use sp_core::crypto::{ByteArray, Pair};
use sp_keystore::Keystore;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use subxt::OnlineClient;
use webb_proposals::TypedChainId;
pub mod errors;
use errors::*;

/// Webb Relayer gadget initialization parameters.
pub struct Eth2LightClientParams {
	/// Concrete local key store
	pub local_keystore: Arc<LocalKeystore>,
	/// Event watching relayer configuration directory
	pub ew_config_dir: Option<PathBuf>,
	/// Light client relayer configuration path
	pub lc_relay_config_path: Option<PathBuf>,
	/// Light client init pallet configuration path
	pub lc_init_config_path: Option<PathBuf>,
	/// Database path
	pub database_path: Option<PathBuf>,
	/// RPC address, `None` if disabled.
	pub rpc_addr: Option<SocketAddr>,
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

	let api = OnlineClient::from_url(lc_relay_config.substrate_endpoint.clone())
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

/// Loads the configuration from the given directory.
fn loads_event_listening_relayer_config(
	config_dir: &PathBuf,
) -> anyhow::Result<webb_relayer_config::WebbRelayerConfig> {
	if !config_dir.is_dir() {
		return Err(InvalidDirectory.into())
	}

	Ok(webb_relayer_config::utils::load(config_dir)?)
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

/// Creates a database store for the relayer based on the configuration passed in.
pub fn create_store(
	database_path: Option<PathBuf>,
) -> anyhow::Result<webb_relayer_store::SledStore> {
	let db_path = match database_path {
		Some(p) => p.join("relayerdb"),
		None => {
			tracing::debug!("Using temp dir for store");
			return webb_relayer_store::SledStore::temporary().map_err(Into::into)
		},
	};

	webb_relayer_store::SledStore::open(db_path).map_err(Into::into)
}

/// Post process the relayer configuration.
///
/// - if there is no signer for any EVM chain, set the signer to the ecdsa key from the
/// keystore.
/// - Ensures that governance relayer is always enabled.
fn post_process_config(
	config: &mut webb_relayer_config::WebbRelayerConfig,
	params: &Eth2LightClientParams,
) -> anyhow::Result<()> {
	// Make sure governance relayer is always enabled
	config.features.governance_relay = true;
	let maybe_ecdsa_pair = get_ecdsa_pair(params.local_keystore.clone())?;
	if maybe_ecdsa_pair.is_none() {
		return Err(FailedToLoadKeys.into())
	}
	let ecdsa_secret = maybe_ecdsa_pair.unwrap().to_raw_vec();
	// for each evm chain, if there is no signer, set the signer to the ecdsa key
	for chain in config.evm.values_mut() {
		if chain.private_key.is_none() {
			chain.private_key = Some(Secret::from_slice(&ecdsa_secret).into())
		}
	}
	Ok(())
}

fn get_ecdsa_pair(local_keystore: Arc<LocalKeystore>) -> anyhow::Result<Option<crypto::Pair>> {
	let maybe_ecdsa_public = local_keystore
		.ecdsa_public_keys(dkg_runtime_primitives::KEY_TYPE)
		.into_iter()
		.find_map(|public_key| crypto::Public::from_slice(&public_key.0).ok());

	if maybe_ecdsa_public.is_none() {
		return Err(FailedToLoadKeys.into())
	}
	local_keystore
		.key_pair::<crypto::Pair>(&maybe_ecdsa_public.unwrap())
		.map_err(Into::into)
}
