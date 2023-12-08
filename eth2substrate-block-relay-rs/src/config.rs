use dotenvy::dotenv;
use eth2_pallet_init::eth_network::EthNetwork;
use eth_rpc_client::beacon_rpc_client::BeaconRPCVersion;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::{io::Read, path::PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
	// endpoint to a full node of Eth2 Beacon chain with Light Client API
	pub beacon_endpoint: String,

	// endpoint for the Ethereum full node, which supports Eth1 RPC API
	pub eth1_endpoint: String,

	// the max number of headers submitted in one batch to eth client
	pub headers_batch_size: u32,

	// endpoint for a full node on the Substrate chain
	pub substrate_endpoint: String,

	// Path to the file with a secret key for signer account
	pub path_to_signer_secret_key: String,

	// The Ethereum network name (Mainnet, Goerli, Sepolia)
	pub ethereum_network: EthNetwork,

	// Period of submission light client updates. Once in N epochs.
	pub interval_between_light_client_updates_submission_in_epochs: u64,

	// maximum gap in slots between submitting light client update
	pub max_blocks_for_finalization: u64,

	// Port for Prometheus
	pub prometheus_metrics_port: Option<u16>,

	// Path to dir for output submitted light client updates and execution blocks
	pub output_dir: Option<String>,

	// Path to the json file with beacon state in the next attested slot
	// for case of short relay run
	pub path_to_attested_state: Option<String>,

	// Include next sync committee to the Light Client Update in short relay run
	pub include_next_sync_committee_to_light_client: bool,

	// Timeout for ETH RPC requests in seconds
	pub eth_requests_timeout_seconds: u64,

	// Timeout for ETH RPC get status requests in seconds
	pub state_requests_timeout_seconds: u64,

	// Timeout for Substrate RPC requests in seconds
	pub substrate_requests_timeout_seconds: u64,

	// Sleep time in seconds when ETH client is synchronized with ETH network
	pub sleep_time_on_sync_secs: u64,

	// Sleep time in seconds after blocks/light_client_update submission to client
	pub sleep_time_after_submission_secs: u64,

	/// Max number of stored blocks in the storage of the eth2 client contract.
	/// Events that happen past this threshold cannot be verified by the client.
	/// It is used on initialization of the Eth2 client.
	pub hashes_gc_threshold: Option<u64>,

	// Beacon rpc version (V1_1, V1_2)
	pub beacon_rpc_version: BeaconRPCVersion,

	pub get_light_client_update_by_epoch: Option<bool>,
}

impl Config {
	pub fn load_from_toml(path: PathBuf) -> Self {
		let mut config = std::fs::File::open(path).expect("Error on parsing path to config");
		let mut content = String::new();
		config.read_to_string(&mut content).expect("Error on reading config");
		let mut config: Config = toml::from_str(content.as_str()).expect("Error on config parsing");
		dotenv().ok();
		let api_key_string = std::env::var("ETH1_INFURA_API_KEY").unwrap();
		config.eth1_endpoint = config.eth1_endpoint.replace("ETH1_INFURA_API_KEY", &api_key_string);

		Self::check_urls(&config);
		config
	}

	fn check_urls(&self) {
		// check `beacon_endpoint`
		Url::parse(&self.beacon_endpoint).expect("Error on beacon endpoint URL parsing");

		// check `eth1_endpoint`
		Url::parse(&self.eth1_endpoint).expect("Error on ETH1 endpoint URL parsing");

		// check `substrate_endpoint`
		Url::parse(&self.substrate_endpoint).expect("Error on Substrate endpoint URL parsing");
	}
}

impl From<Config> for eth2_pallet_init::config::Config {
	fn from(val: Config) -> Self {
		eth2_pallet_init::config::Config {
			beacon_endpoint: val.beacon_endpoint,
			eth1_endpoint: val.eth1_endpoint,
			substrate_endpoint: val.substrate_endpoint,
			path_to_signer_secret_key: val.path_to_signer_secret_key,
			ethereum_network: val.ethereum_network,
			output_dir: val.output_dir,
			eth_requests_timeout_seconds: Some(val.eth_requests_timeout_seconds),
			validate_updates: None,
			verify_bls_signature: None,
			hashes_gc_threshold: val.hashes_gc_threshold,
			trusted_signer_account_id: None,
			init_block_root: None,
			beacon_rpc_version: val.beacon_rpc_version,
		}
	}
}
