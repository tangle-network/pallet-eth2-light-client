use crate::{
	config::Config, substrate_network::SubstrateNetwork, substrate_pallet_client::EthClientPallet,
};
use eth_rpc_client::{
	beacon_rpc_client::BeaconRPCClient, eth1_rpc_client::Eth1RPCClient,
	light_client_snapshot_with_proof::LightClientSnapshotWithProof,
};
use eth_types::{eth2::ExtendedBeaconBlockHeader, BlockHeader};
use log::info;
use sp_core::crypto::AccountId32;
use std::time;
use tree_hash::TreeHash;
use webb_proposals::TypedChainId;

const CURRENT_SYNC_COMMITTEE_INDEX: u32 = 54;
const CURRENT_SYNC_COMMITTEE_TREE_DEPTH: u32 =
	consensus_types::floorlog2(CURRENT_SYNC_COMMITTEE_INDEX);
const CURRENT_SYNC_COMMITTEE_TREE_INDEX: u32 =
	consensus_types::get_subtree_index(CURRENT_SYNC_COMMITTEE_INDEX);

pub fn verify_light_client_snapshot(
	block_root: String,
	light_client_snapshot: &LightClientSnapshotWithProof,
) -> bool {
	let expected_block_root =
		format!("{:#x}", light_client_snapshot.beacon_header.tree_hash_root());

	if block_root != expected_block_root {
		return false
	}

	let branch =
		consensus_types::convert_branch(&light_client_snapshot.current_sync_committee_branch);
	merkle_proof::verify_merkle_proof(
		light_client_snapshot.current_sync_committee.tree_hash_root(),
		&branch,
		CURRENT_SYNC_COMMITTEE_TREE_DEPTH.try_into().unwrap(),
		CURRENT_SYNC_COMMITTEE_TREE_INDEX.try_into().unwrap(),
		light_client_snapshot.beacon_header.state_root.0,
	)
}

pub async fn init_pallet(
	config: &Config,
	eth_client_pallet: &mut EthClientPallet,
) -> Result<(), Box<dyn std::error::Error>> {
	info!(target: "relay", "=== Contract initialization ===");
	if let SubstrateNetwork::Mainnet = config.substrate_network_id {
		assert!(
			config.validate_updates.unwrap_or(true),
			"The updates validation can't be disabled for mainnet"
		);
		assert!(config.verify_bls_signature.unwrap_or(false) || config.trusted_signer_account_id.is_some(), "The client can't be executed in the trustless mode without BLS sigs verification on Mainnet");
	}
	info!(target: "relay", "=== Contract initialization RB0 ===");

	let beacon_rpc_client = BeaconRPCClient::new(
		&config.beacon_endpoint,
		config.eth_requests_timeout_seconds.unwrap_or(10),
		config.eth_requests_timeout_seconds.unwrap_or(10),
		Some(config.beacon_rpc_version.clone()),
	);
	info!(target: "relay", "=== Contract initialization RB1 ===");
	let eth1_rpc_client = Eth1RPCClient::new(&config.eth1_endpoint);

	info!(target: "relay", "=== Contract initialization RB2 ===");
	let light_client_update_with_next_sync_committee = beacon_rpc_client
		.get_light_client_update_for_last_period()
		.await
		.expect("Error on fetching finality light client update with sync committee update");
		info!(target: "relay", "=== Contract initialization RB3 ===");
	let finality_light_client_update = beacon_rpc_client
		.get_finality_light_client_update()
		.await
		.expect("Error on fetching finality light client update");
		info!(target: "relay", "=== Contract initialization RB4 ===");

	let finality_slot =
		finality_light_client_update.finality_update.header_update.beacon_header.slot;

		info!(target: "relay", "=== Contract initialization RB5 ===");

	let block_id = format!("{}", finality_slot);

	let finalized_header: ExtendedBeaconBlockHeader =
		ExtendedBeaconBlockHeader::from(finality_light_client_update.finality_update.header_update);
		info!(target: "relay", "=== Contract initialization RB6 ===");
	let finalized_body = beacon_rpc_client
		.get_beacon_block_body_for_block_id(&block_id)
		.await
		.expect("Error on fetching finalized body");
		info!(target: "relay", "=== Contract initialization RB7 ===");

	let finalized_execution_header: BlockHeader = eth1_rpc_client
		.get_block_header_by_number(
			finalized_body
				.execution_payload()
				.expect("No execution payload in finalized body")
				.execution_payload
				.block_number,
		)
		.await
		.expect("Error on fetching finalized execution header");

		info!(target: "relay", "=== Contract initialization RB8 ===");

	let next_sync_committee = light_client_update_with_next_sync_committee
		.sync_committee_update
		.expect("No sync_committee update in light client update")
		.next_sync_committee;

		info!(target: "relay", "=== Contract initialization RB9 ===");

	let init_block_root = match config.init_block_root.clone() {
		None => beacon_rpc_client.get_checkpoint_root().await.expect("Fail to get last checkpoint"),
		Some(init_block_str) => init_block_str,
	};

	info!(target: "relay", "=== Contract initialization RB10 ===");

	let light_client_snapshot = beacon_rpc_client
		.get_bootstrap(init_block_root.clone())
		.await
		.expect("Unable to fetch bootstrap state");

		info!(target: "relay", "=== Contract initialization RB11 ===");

	info!(target: "relay", "init_block_root: {}", init_block_root);

	if BeaconRPCClient::get_period_for_slot(light_client_snapshot.beacon_header.slot) !=
		BeaconRPCClient::get_period_for_slot(finality_slot)
	{
		panic!("Period for init_block_root different from current period. Please use snapshot for current period");
	}

	if !verify_light_client_snapshot(init_block_root, &light_client_snapshot) {
		return Err("Invalid light client snapshot".into())
	}

	let mut trusted_signature: Option<AccountId32> = Option::None;
	if let Some(trusted_signature_name) = config.trusted_signer_account_id.clone() {
		trusted_signature = Option::Some(
			trusted_signature_name
				.parse()
				.expect("Error on parsing trusted signature account"),
		);
	}

	info!(target: "relay", "=== Contract initialization RB12 ===");

	let typed_chain_id = match config.ethereum_network {
		crate::eth_network::EthNetwork::Mainnet => TypedChainId::Evm(1),
		crate::eth_network::EthNetwork::Kiln => TypedChainId::Evm(1337802),
		crate::eth_network::EthNetwork::Ropsten => TypedChainId::Evm(3),
		crate::eth_network::EthNetwork::Goerli => TypedChainId::Evm(5),
	};

	info!(target: "relay", "=== Contract initialization RB13 ===");

	eth_client_pallet
		.init(
			typed_chain_id,
			finalized_execution_header,
			finalized_header,
			light_client_snapshot.current_sync_committee,
			next_sync_committee,
			config.validate_updates,
			config.verify_bls_signature,
			config.hashes_gc_threshold,
			config.max_submitted_blocks_by_account,
			trusted_signature,
		)
		.await
		.unwrap();

		info!(target: "relay", "=== Contract initialization RB14 ===");

	tokio::time::sleep(time::Duration::from_secs(30)).await;
	Ok(())
}

#[cfg(test)]
mod tests {
	use crate::{
		config_for_tests::ConfigForTests,
		eth_client_pallet_trait::EthClientPalletTrait,
		init_pallet::init_pallet,
		substrate_network::SubstrateNetwork,
		substrate_pallet_client::{setup_api, EthClientPallet},
	};
	use eth_rpc_client::beacon_rpc_client::{BeaconRPCClient, BeaconRPCVersion};

	const ONE_EPOCH_IN_SLOTS: u64 = 32;

	fn get_init_config(
		config_for_test: &ConfigForTests,
		eth_client_pallet: &EthClientPallet,
	) -> crate::config::Config {
		return crate::config::Config {
			beacon_endpoint: config_for_test.beacon_endpoint.to_string(),
			eth1_endpoint: config_for_test.eth1_endpoint.to_string(),
			substrate_endpoint: "https://localhost:9944".to_string(),
			signer_account_id: "NaN".to_string(),
			path_to_signer_secret_key: "NaN".to_string(),
			contract_account_id: "NaN".to_string(),
			ethereum_network: config_for_test.network_name.clone(),
			substrate_network_id: SubstrateNetwork::Testnet,
			output_dir: None,
			eth_requests_timeout_seconds: Some(30),
			validate_updates: Some(true),
			verify_bls_signature: Some(false),
			hashes_gc_threshold: Some(51000),
			max_submitted_blocks_by_account: Some(8000),
			trusted_signer_account_id: Some(eth_client_pallet.get_signer_account_id().to_string()),
			init_block_root: None,
			beacon_rpc_version: BeaconRPCVersion::V1_1,
		}
	}

	#[tokio::test]
	#[should_panic(expected = "The updates validation can't be disabled for mainnet")]
	async fn test_init_pallet_on_mainnet_without_validation() {
		let config_for_test =
			ConfigForTests::load_from_toml("config_for_tests.toml".try_into().unwrap());

		let api = setup_api().await.unwrap();
		let mut eth_client_pallet = EthClientPallet::new(api);
		let mut init_config = get_init_config(&config_for_test, &eth_client_pallet);
		init_config.validate_updates = Some(false);
		init_config.substrate_network_id = SubstrateNetwork::Testnet;

		init_pallet(&init_config, &mut eth_client_pallet).await.unwrap();
	}

	#[tokio::test]
	#[should_panic(
		expected = "The client can't be executed in the trustless mode without BLS sigs verification on Mainnet"
	)]
	async fn test_init_pallet_on_mainnet_without_trusted_signature() {
		let config_for_test =
			ConfigForTests::load_from_toml("config_for_tests.toml".try_into().unwrap());

		let api = setup_api().await.unwrap();
		let mut eth_client_pallet = EthClientPallet::new(api);
		let mut init_config = get_init_config(&config_for_test, &eth_client_pallet);
		init_config.substrate_network_id = SubstrateNetwork::Testnet;
		init_config.trusted_signer_account_id = None;

		init_pallet(&init_config, &mut eth_client_pallet).await.unwrap();
	}

	#[tokio::test]
	async fn test_sync_with_eth_after_init() {
		let config_for_test =
			ConfigForTests::load_from_toml("config_for_tests.toml".try_into().unwrap());

		let api = setup_api().await.unwrap();
		let mut eth_client_pallet = EthClientPallet::new(api);
		let init_config = get_init_config(&config_for_test, &eth_client_pallet);

		init_pallet(&init_config, &mut eth_client_pallet).await.unwrap();

		let last_finalized_slot_eth_client = eth_client_pallet
			.get_finalized_beacon_block_slot()
			.await
			.expect("Error on getting last finalized beacon block slot(Eth client)");

		let beacon_rpc_client = BeaconRPCClient::new(
			&init_config.beacon_endpoint,
			init_config.eth_requests_timeout_seconds.unwrap_or(10),
			init_config.eth_requests_timeout_seconds.unwrap_or(10),
			None,
		);

		let last_finalized_slot_eth_network = beacon_rpc_client
			.get_last_finalized_slot_number()
			.expect("Error on getting last finalized beacon block slot");

		const MAX_GAP_IN_EPOCH_BETWEEN_FINALIZED_SLOTS: u64 = 3;

		assert!(
			last_finalized_slot_eth_client +
				ONE_EPOCH_IN_SLOTS * MAX_GAP_IN_EPOCH_BETWEEN_FINALIZED_SLOTS >=
				last_finalized_slot_eth_network
		);
	}
}
