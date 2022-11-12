use crate::contract_type::ContractType;
use eth_rpc_client::beacon_rpc_client::BeaconRPCVersion;
use reqwest::Url;
use serde::Deserialize;
use std::io::Read;
use std::path::PathBuf;

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    // endpoint to a full node of Eth2 Beacon chain with Light Client API
    pub beacon_endpoint: String,

    // endpoint for the Ethereum full node, which supports Eth1 RPC API
    pub eth1_endpoint: String,

    // the max number of headers submitted in one batch to eth client
    pub headers_batch_size: u32,

    // endpoint for a full node on the Substrate chain
    pub substrate_endpoint: String,

    // Account id from which relay make requests
    pub signer_account_id: String,

    // Path to the file with a secret key for signer account
    pub path_to_signer_secret_key: String,

    // Account id for eth client contract on Substrate
    pub contract_account_id: String,

    // The Ethereum network name (Mainnet, Kiln, Ropsten, Goerli)
    pub ethereum_network: String,

    // Contract type (near, dao, file)
    pub contract_type: ContractType,

    // Period of submission light client updates. Once in N epochs.
    pub interval_between_light_client_updates_submission_in_epochs: u64,

    // maximum gap in slots between submitting light client update
    pub max_blocks_for_finalization: u64,

    // Port for Prometheus
    pub prometheus_metrics_port: Option<u16>,

    // Account id for DAO on Substrate
    pub dao_contract_account_id: Option<String>,

    // Path to dir for output submitted light client updates and execution blocks
    pub output_dir: Option<String>,

    // Path to the json file with beacon state in the next attested slot
    // for case of short relay run
    pub path_to_attested_state: Option<String>,

    // Path to the json file with beacon state in the next finality slot
    // for case of short relay run
    pub path_to_finality_state: Option<String>,

    // Timeout for ETH RPC requests in seconds
    pub eth_requests_timeout_seconds: u64,

    // Timeout for ETH RPC get status requests in seconds
    pub state_requests_timeout_seconds: u64,

    // Sleep time in seconds when ETH client is synchronized with ETH network
    pub sleep_time_on_sync_secs: u64,

    // Sleep time in seconds after blocks/light_client_update submission to client
    pub sleep_time_after_submission_secs: u64,

    /// Max number of stored blocks in the storage of the eth2 client contract.
    /// Events that happen past this threshold cannot be verified by the client.
    /// It is used on initialization of the Eth2 client.
    pub hashes_gc_threshold: Option<u64>,

    /// Max number of unfinalized blocks allowed to be stored by one submitter account.
    /// It is used on initialization of the Eth2 client.
    pub max_submitted_blocks_by_account: Option<u32>,

    // Beacon rpc version (V1_1, V1_2)
    pub beacon_rpc_version: BeaconRPCVersion,
}

impl Config {
    pub fn load_from_toml(path: PathBuf) -> Self {
        let mut config = std::fs::File::open(path).expect("Error on parsing path to config");
        let mut content = String::new();
        config.read_to_string(&mut content).expect("Error on reading config");
        let config = toml::from_str(content.as_str()).expect("Error on config parsing");

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
