use crate::{
	config::Config, config_for_tests::ConfigForTests, eth2substrate_relay::Eth2SubstrateRelay,
	test_utils,
};
use bitvec::macros::internal::funty::Fundamental;
use eth2_pallet_init::{
	eth_client_pallet_trait::EthClientPalletTrait,
	eth_network::EthNetwork,
	init_pallet,
	init_pallet::init_pallet,
	substrate_pallet_client::{setup_api, EthClientPallet},
};
use eth_rpc_client::{
	beacon_rpc_client::{BeaconRPCClient, BeaconRPCVersion},
	eth1_rpc_client::Eth1RPCClient,
};
use eth_types::{
	eth2::{ExtendedBeaconBlockHeader, LightClientUpdate, SyncCommittee},
	BlockHeader,
};
use std::{thread, time};
use tree_hash::TreeHash;
use webb::substrate::subxt::tx::PairSigner;
use webb_proposals::TypedChainId;

pub fn read_json_file_from_data_dir(file_name: &str) -> std::string::String {
	let mut json_file_path = std::env::current_exe().unwrap();
	json_file_path.pop();
	json_file_path.push("../../../data");
	json_file_path.push(file_name);

	std::fs::read_to_string(json_file_path).expect("Unable to read file")
}

pub async fn init_pallet_from_files(
	eth_client_pallet: &mut EthClientPallet,
	config_for_test: &ConfigForTests,
) {
	let execution_blocks: Vec<BlockHeader> = serde_json::from_str(
		&std::fs::read_to_string(config_for_test.path_to_execution_blocks_headers.clone())
			.expect("Unable to read file"),
	)
	.unwrap();

	let light_client_updates: Vec<LightClientUpdate> = serde_json::from_str(
		&std::fs::read_to_string(config_for_test.path_to_light_client_updates.clone())
			.expect("Unable to read file"),
	)
	.unwrap();

	let current_sync_committee: SyncCommittee = serde_json::from_str(
		&std::fs::read_to_string(config_for_test.path_to_current_sync_committee.clone())
			.expect("Unable to read file"),
	)
	.unwrap();
	let next_sync_committee: SyncCommittee = serde_json::from_str(
		&std::fs::read_to_string(config_for_test.path_to_next_sync_committee.clone())
			.expect("Unable to read file"),
	)
	.unwrap();

	let finalized_beacon_header = ExtendedBeaconBlockHeader::from(
		light_client_updates[0].clone().finality_update.header_update,
	);

	let finalized_hash = light_client_updates[0]
		.clone()
		.finality_update
		.header_update
		.execution_block_hash;
	let mut finalized_execution_header = None::<BlockHeader>;
	for header in &execution_blocks {
		if header.hash.unwrap() == finalized_hash {
			finalized_execution_header = Some(header.clone());
			break
		}
	}

	let typed_chain_id = match config_for_test.network_name.clone() {
		EthNetwork::Mainnet => TypedChainId::Evm(1),
		EthNetwork::Kiln => TypedChainId::Evm(1337802),
		EthNetwork::Ropsten => TypedChainId::Evm(3),
		EthNetwork::Goerli => TypedChainId::Evm(5),
	};

	eth_client_pallet
		.init(
			typed_chain_id,
			finalized_execution_header.unwrap(),
			finalized_beacon_header,
			current_sync_committee,
			next_sync_committee,
			Some(true),
			Some(false),
			None,
			None,
			Some(eth_client_pallet.get_signer_account_id()),
		)
		.await
		.unwrap();

	thread::sleep(time::Duration::from_secs(30));

	eth_client_pallet
}

pub async fn init_pallet_from_specific_slot(
	eth_client_pallet: &mut EthClientPallet,
	finality_slot: u64,
	config_for_test: &ConfigForTests,
) {
	const TIMEOUT: u64 = 30;
	const TIMEOUT_STATE: u64 = 1000;

	let current_sync_committee: SyncCommittee = serde_json::from_str(
		&std::fs::read_to_string(config_for_test.path_to_current_sync_committee.clone())
			.expect("Unable to read file"),
	)
	.unwrap();
	let next_sync_committee: SyncCommittee = serde_json::from_str(
		&std::fs::read_to_string(config_for_test.path_to_next_sync_committee.clone())
			.expect("Unable to read file"),
	)
	.unwrap();

	let beacon_rpc_client =
		BeaconRPCClient::new(&config_for_test.beacon_endpoint, TIMEOUT, TIMEOUT_STATE, None);
	let eth1_rpc_client = Eth1RPCClient::new(&config_for_test.eth1_endpoint);

	let finality_header = beacon_rpc_client
		.get_beacon_block_header_for_block_id(&format!("{}", finality_slot))
		.unwrap();

	let finality_header = eth_types::eth2::BeaconBlockHeader {
		slot: finality_header.slot.as_u64(),
		proposer_index: finality_header.proposer_index,
		parent_root: finality_header.parent_root.into(),
		state_root: finality_header.state_root.into(),
		body_root: finality_header.body_root.into(),
	};

	let finalized_body = beacon_rpc_client
		.get_beacon_block_body_for_block_id(&format!("{}", finality_slot))
		.unwrap();

	let finalized_beacon_header = ExtendedBeaconBlockHeader {
		header: finality_header.clone(),
		beacon_block_root: eth_types::H256(finality_header.tree_hash_root()),
		execution_block_hash: finalized_body
			.execution_payload()
			.unwrap()
			.execution_payload
			.block_hash
			.into_root()
			.into(),
	};

	let finalized_execution_header: BlockHeader = eth1_rpc_client
		.get_block_header_by_number(
			finalized_body.execution_payload().unwrap().execution_payload.block_number,
		)
		.unwrap();

	eth_client_pallet
		.init(
			config_for_test.network_name.clone(),
			finalized_execution_header,
			finalized_beacon_header,
			current_sync_committee,
			next_sync_committee,
			Some(true),
			Some(false),
			None,
			None,
			Some(eth_client_pallet.get_signer_account_id()),
		)
		.await;

	thread::sleep(time::Duration::from_secs(30));
}

fn get_config(config_for_test: &ConfigForTests) -> Config {
	Config {
		beacon_endpoint: config_for_test.beacon_endpoint.to_string(),
		eth1_endpoint: config_for_test.eth1_endpoint.to_string(),
		headers_batch_size: 8,
		signer_account_id: "NaN".to_string(),
		path_to_signer_secret_key: "NaN".to_string(),
		contract_account_id: "NaN".to_string(),
		ethereum_network: config_for_test.network_name.clone(),
		interval_between_light_client_updates_submission_in_epochs: 1,
		max_blocks_for_finalization: 5000,
		prometheus_metrics_port: Some(32221),
		output_dir: None,
		path_to_attested_state: None,
		path_to_finality_state: None,
		eth_requests_timeout_seconds: 30,
		state_requests_timeout_seconds: 1000,
		sleep_time_on_sync_secs: 0,
		sleep_time_after_submission_secs: 5,
		hashes_gc_threshold: None,
		max_submitted_blocks_by_account: None,
		beacon_rpc_version: BeaconRPCVersion::V1_1,
		substrate_endpoint: "localhost:9944".to_string(),
		substrate_network_name: "Tangle Testnet".to_string(),
	}
}

fn get_init_config(
	config_for_test: &ConfigForTests,
	eth_client_pallet: &EthClientPallet,
) -> eth2_pallet_init::config::Config {
	eth2_pallet_init::config::Config {
		beacon_endpoint: config_for_test.beacon_endpoint.to_string(),
		eth1_endpoint: config_for_test.eth1_endpoint.to_string(),
		signer_account_id: "alice".to_string(),
		path_to_signer_secret_key: "NaN".to_string(),
		contract_account_id: "NaN".to_string(),
		ethereum_network: config_for_test.network_name.clone(),
		output_dir: None,
		eth_requests_timeout_seconds: Some(30),
		validate_updates: Some(true),
		verify_bls_signature: Some(false),
		hashes_gc_threshold: Some(51000),
		max_submitted_blocks_by_account: Some(8000),
		trusted_signer_account_id: Some(eth_client_pallet.get_signer_account_id().to_string()),
		init_block_root: None,
		beacon_rpc_version: BeaconRPCVersion::V1_1,
		substrate_endpoint: "localhost:9944".to_string(),
		substrate_network_id: 1080,
	}
}

pub async fn get_client_pallet(
	from_file: bool,
	config_for_test: &ConfigForTests,
) -> Box<dyn EthClientPalletTrait> {
	let api = setup_api().await.unwrap();
	let mut eth_client_pallet = EthClientPallet::new(api);

	let mut config = get_init_config(config_for_test, eth_client_pallet);
	config.signer_account_id = eth_client_pallet.get_signer_account_id().to_string();

	match from_file {
		true => test_utils::init_pallet_from_files(&mut eth_client_pallet, config_for_test).await,
		false => init_pallet(&config, &mut eth_client_pallet).await.unwrap(),
	};

	Box::new(eth_client_pallet)
}

pub async fn get_relay(
	enable_binsearch: bool,
	from_file: bool,
	config_for_test: &ConfigForTests,
) -> Eth2SubstrateRelay {
	let config = get_config(config_for_test);
	Eth2SubstrateRelay::init(
		&config,
		get_client_pallet(from_file, config_for_test).await,
		enable_binsearch,
		false,
	)
	.await
}

pub async fn get_relay_with_update_from_file(
	enable_binsearch: bool,
	from_file: bool,
	next_sync_committee: bool,
	config_for_test: &ConfigForTests,
) -> Eth2SubstrateRelay {
	let mut config = get_config(config_for_test);
	config.path_to_attested_state = Some(config_for_test.path_to_attested_state.to_string());

	if next_sync_committee {
		config.path_to_finality_state = Some(config_for_test.path_to_finality_state.to_string());
	}

	Eth2SubstrateRelay::init(
		&config,
		get_client_pallet(from_file, config_for_test).await,
		enable_binsearch,
		false,
	)
	.await
}

pub async fn get_relay_from_slot(
	enable_binsearch: bool,
	slot: u64,
	config_for_test: &ConfigForTests,
) -> Eth2SubstrateRelay {
	let config = get_config(config_for_test);
	let api = setup_api().await.unwrap();
	let mut eth_client_pallet = EthClientPallet::new(api);

	init_pallet_from_specific_slot(&mut eth_client_pallet, slot, config_for_test);

	Eth2SubstrateRelay::init(&config, Box::new(eth_client_pallet), enable_binsearch, false).await
}