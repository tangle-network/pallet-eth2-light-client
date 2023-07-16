use crate::{
	errors::{
		ErrorOnJsonParse, ExecutionPayloadError, FailOnGettingJson, MissSyncAggregationError,
		NoBlockForSlotError, SignatureSlotNotFoundError,
	},
	execution_block_proof::ExecutionBlockProof,
	light_client_snapshot_with_proof::LightClientSnapshotWithProof,
	utils,
};
use eth_types::{
	eth2::{
		BeaconBlockHeader, FinalizedHeaderUpdate, HeaderUpdate, LightClientUpdate, Slot,
		SyncAggregate, SyncCommittee, SyncCommitteeUpdate,
	},
	H256,
};
use funty::Fundamental;
use log::trace;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{string::String, time::Duration};
use types::{BeaconBlockBody, BeaconState, MainnetEthSpec};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BeaconRPCVersion {
	V1_1,
	V1_2,
}

struct BeaconRPCRoutes {
	pub get_block_header: String,
	pub get_block: String,
	pub get_light_client_update: String,
	pub get_light_client_finality_update: String,
	pub get_bootstrap: String,
	pub get_state: String,
}

impl BeaconRPCRoutes {
	pub fn new(version: BeaconRPCVersion) -> Self {
		match version {
			BeaconRPCVersion::V1_1 => Self {
				get_block_header: "eth/v1/beacon/headers".to_string(),
				get_block: "eth/v2/beacon/blocks".to_string(),
				get_light_client_update: "eth/v1/beacon/light_client/updates".to_string(),
				get_light_client_finality_update: "eth/v1/beacon/light_client/finality_update/"
					.to_string(),
				get_bootstrap: "eth/v1/beacon/light_client/bootstrap".to_string(),
				get_state: "eth/v2/debug/beacon/states".to_string(),
			},
			BeaconRPCVersion::V1_2 => Self {
				get_block_header: "eth/v1/beacon/headers".to_string(),
				get_block: "eth/v2/beacon/blocks".to_string(),
				get_light_client_update: "eth/v1/beacon/light_client/updates".to_string(),
				get_light_client_finality_update: "eth/v1/beacon/light_client/finality_update"
					.to_string(),
				get_bootstrap: "eth/v1/beacon/light_client/bootstrap".to_string(),
				get_state: "eth/v2/debug/beacon/states".to_string(),
			},
		}
	}
}

/// `BeaconRPCClient` allows getting beacon block body, beacon block header
/// and light client updates
/// using Beacon RPC API (https://ethereum.github.io/beacon-APIs/)
pub struct BeaconRPCClient {
	endpoint_url: String,
	client: Client,
	client_state_request: Client,
	routes: BeaconRPCRoutes,
}

impl BeaconRPCClient {
	const SLOTS_PER_EPOCH: u64 = 32;
	const EPOCHS_PER_PERIOD: u64 = 256;

	/// Creates `BeaconRPCClient` for the given BeaconAPI `endpoint_url`
	pub fn new(
		endpoint_url: &str,
		timeout_seconds: u64,
		timeout_state_seconds: u64,
		version: Option<BeaconRPCVersion>,
	) -> Self {
		Self {
			endpoint_url: endpoint_url.to_string(),
			client: reqwest::Client::builder()
				.timeout(Duration::from_secs(timeout_seconds))
				.build()
				.expect("Error on building blocking client for regular rpc requests."),
			client_state_request: reqwest::Client::builder()
				.timeout(Duration::from_secs(timeout_state_seconds))
				.build()
				.expect("Error on building blocking client for state request."),
			routes: BeaconRPCRoutes::new(version.unwrap_or(BeaconRPCVersion::V1_1)),
		}
	}

	/// Returns `BeaconBlockBody` struct for the given `block_id`.
	///
	/// # Arguments
	///
	/// * `block_id` - Block identifier. Can be one of: "head" (canonical head in node's view),
	/// "genesis", "finalized", <slot>, <hex encoded blockRoot with 0x prefix>
	/// (see https://ethereum.github.io/beacon-APIs/#/Beacon/getBlockV2)
	pub async fn get_beacon_block_body_for_block_id(
		&self,
		block_id: &str,
	) -> Result<BeaconBlockBody<MainnetEthSpec>, crate::Error> {
		let url = format!("{}/{}/{}", self.endpoint_url, self.routes.get_block, block_id);

		let json_str = &self.get_json_from_raw_request(&url).await?;

		self.check_block_found_for_slot(json_str)?;
		let body_json = &Self::get_body_json_from_rpc_result(json_str)?;

		Ok(serde_json::from_str(body_json)?)
	}

	/// Returns `BeaconBlockHeader` struct for the given `block_id`.
	///
	/// # Arguments
	///
	/// * `block_id` - Block identifier. Can be one of: "head" (canonical head in node's view),
	/// "genesis", "finalized", <slot>, <hex encoded blockRoot with 0x prefix>
	/// (see https://ethereum.github.io/beacon-APIs/#/Beacon/getBlockHeader)
	pub async fn get_beacon_block_header_for_block_id(
		&self,
		block_id: &str,
	) -> Result<BeaconBlockHeader, crate::Error> {
		let url = format!("{}/{}/{}", self.endpoint_url, self.routes.get_block_header, block_id);

		let json_str = &self.get_json_from_raw_request(&url).await?;
		self.check_block_found_for_slot(json_str)?;
		let json_str = Self::get_header_json_from_rpc_result(json_str)?;
		Ok(serde_json::from_str(&json_str)?)
	}

	/// Returns `LightClientUpdate` struct for the given `period`.
	///
	/// # Arguments
	///
	/// * `period` - period id for which `LightClientUpdate` is fetched.
	/// On Mainnet, one period consists of 256 epochs, and one epoch consists of 32 slots
	pub async fn get_light_client_update(
		&self,
		period: u64,
	) -> Result<LightClientUpdate, crate::Error> {
		let url = format!(
			"{}/{}?start_period={}&count=1",
			self.endpoint_url, self.routes.get_light_client_update, period
		);
		println!("url: {url}");
		let light_client_update_json_str = self.get_json_from_raw_request(&url).await?;
		println!("light_client_update_json_str: {light_client_update_json_str}");
		let attested_beacon_header = Self::get_attested_header_from_light_client_update_json_str(
			&light_client_update_json_str,
		)?;

		let sync_aggregate = Self::get_sync_aggregate_from_light_client_update_json_str(
			&light_client_update_json_str,
		)?;

		let signature_slot = self.get_signature_slot(&light_client_update_json_str).await?;

		let finality_update = self
			.get_finality_update_from_light_client_update_json_str(&light_client_update_json_str)
			.await?;

		let sync_committee_update =
			Some(Self::get_sync_committee_update_from_light_client_update_json_str(
				&light_client_update_json_str,
			)?);

		Ok(LightClientUpdate {
			attested_beacon_header,
			sync_aggregate,
			signature_slot,
			finality_update,
			sync_committee_update,
		})
	}

	// Fetch a bootstrapping state with a proof to a trusted block root.
	// The trusted block root should be fetched with similar means to a weak subjectivity
	// checkpoint. Only block roots for checkpoints are guaranteed to be available.
	pub async fn get_bootstrap(
		&self,
		block_root: String,
	) -> Result<LightClientSnapshotWithProof, crate::Error> {
		let url = format!("{}/{}/{}", self.endpoint_url, self.routes.get_bootstrap, block_root);

		let light_client_snapshot_json_str = self.get_json_from_raw_request(&url).await?;
		println!("LCU: {light_client_snapshot_json_str}");
		let parsed_json: Value = serde_json::from_str(&light_client_snapshot_json_str)?;

		let beacon_header: BeaconBlockHeader =
			serde_json::from_value(parsed_json["data"]["header"]["beacon"].clone())?;
		let current_sync_committee: SyncCommittee =
			serde_json::from_value(parsed_json["data"]["current_sync_committee"].clone())?;
		let current_sync_committee_branch: Vec<H256> =
			serde_json::from_value(parsed_json["data"]["current_sync_committee_branch"].clone())?;

		Ok(LightClientSnapshotWithProof {
			beacon_header,
			current_sync_committee,
			current_sync_committee_branch,
		})
	}

	pub async fn get_checkpoint_root(&self) -> Result<String, crate::Error> {
		let url =
			format!("{}/eth/v1/beacon/states/finalized/finality_checkpoints", self.endpoint_url);
		let checkpoint_json_str = self.get_json_from_raw_request(&url).await?;
		let parsed_json: Value = serde_json::from_str(&checkpoint_json_str)?;

		Ok(utils::trim_quotes(parsed_json["data"]["finalized"]["root"].to_string()))
	}

	/// Return the last finalized slot in the Beacon chain
	pub async fn get_last_finalized_slot_number(&self) -> Result<Slot, crate::Error> {
		Ok(self.get_beacon_block_header_for_block_id("finalized").await?.slot)
	}

	/// Return the last slot in the Beacon chain
	pub async fn get_last_slot_number(&self) -> Result<Slot, crate::Error> {
		Ok(self.get_beacon_block_header_for_block_id("head").await?.slot)
	}

	pub async fn get_slot_by_beacon_block_root(
		&self,
		beacon_block_hash: H256,
	) -> Result<u64, crate::Error> {
		let beacon_block_hash_str: String =
			utils::trim_quotes(serde_json::to_string(&beacon_block_hash)?);

		let url =
			format!("{}/{}/{}", self.endpoint_url, self.routes.get_block, beacon_block_hash_str);
		let block_json_str = &self.get_json_from_raw_request(&url).await?;
		let v: Value = serde_json::from_str(block_json_str)?;
		let slot = utils::trim_quotes(v["data"]["message"]["slot"].to_string()).parse::<u64>()?;

		Ok(slot)
	}

	pub async fn get_block_number_for_slot(&self, slot: Slot) -> Result<u64, crate::Error> {
		let beacon_block_body = self.get_beacon_block_body_for_block_id(&slot.to_string()).await?;
		Ok(beacon_block_body
			.execution_payload()
			.map_err(|_| ExecutionPayloadError)?
			.execution_payload_ref()
			.block_number())
	}

	pub async fn get_finality_light_client_update(
		&self,
	) -> Result<LightClientUpdate, crate::Error> {
		let url =
			format!("{}/{}", self.endpoint_url, self.routes.get_light_client_finality_update,);

		let light_client_update_json_str = self.get_json_from_raw_request(&url).await?;
		let v: Value = serde_json::from_str(&light_client_update_json_str)?;
		let light_client_update_json_str = serde_json::to_string(&json!([v]))?;

		Ok(LightClientUpdate {
			attested_beacon_header: Self::get_attested_header_from_light_client_update_json_str(
				&light_client_update_json_str,
			)?,
			sync_aggregate: Self::get_sync_aggregate_from_light_client_update_json_str(
				&light_client_update_json_str,
			)?,
			signature_slot: self.get_signature_slot(&light_client_update_json_str).await?,
			finality_update: self
				.get_finality_update_from_light_client_update_json_str(
					&light_client_update_json_str,
				)
				.await?,
			sync_committee_update: None::<SyncCommitteeUpdate>,
		})
	}

	/// Returns the best light client update for the last period
	///
	/// Best is defined by (in order of priority):
	/// - Is finalized update
	/// - Has most bits
	/// - Oldest update
	pub async fn get_light_client_update_for_last_period(
		&self,
	) -> Result<LightClientUpdate, crate::Error> {
		let last_slot = self.get_last_slot_number().await?.as_u64();
		let last_period = Self::get_period_for_slot(last_slot);
		self.get_light_client_update(last_period).await
	}

	pub async fn get_beacon_state(
		&self,
		state_id: &str,
	) -> Result<BeaconState<MainnetEthSpec>, crate::Error> {
		let url_request = format!("{}/{}/{}", self.endpoint_url, self.routes.get_state, state_id);
		let json_str = Self::get_json_from_client(&self.client_state_request, &url_request).await?;

		let v: Value = serde_json::from_str(&json_str)?;
		let state_json_str = serde_json::to_string(&v["data"])?;

		Ok(serde_json::from_str(&state_json_str)?)
	}

	pub async fn is_syncing(&self) -> Result<bool, crate::Error> {
		let url_request = format!("{}/eth/v1/node/syncing", self.endpoint_url);
		let json_str = self.get_json_from_raw_request(&url_request).await?;
		log::info!(target: "relay", "Beacon RPC client syncing status: {json_str}");
		let v: Value = serde_json::from_str(&json_str)?;
		v["data"]["is_syncing"].as_bool().ok_or_else(|| ErrorOnJsonParse.into())
	}

	async fn get_json_from_client(client: &Client, url: &str) -> Result<String, crate::Error> {
		trace!(target: "relay", "Beacon chain request: {}", url);
		let json_str = client.get(url).send().await?.text().await?;
		if serde_json::from_str::<Value>(&json_str).is_err() {
			return Err(FailOnGettingJson { response: json_str }.into())
		}

		Ok(json_str)
	}

	async fn get_json_from_raw_request(&self, url: &str) -> Result<String, crate::Error> {
		Self::get_json_from_client(&self.client, url).await
	}

	fn get_body_json_from_rpc_result(
		block_json_str: &str,
	) -> Result<std::string::String, crate::Error> {
		let v: Value = serde_json::from_str(block_json_str)?;
		let body_json_str = serde_json::to_string(&v["data"]["message"]["body"])?;
		Ok(body_json_str)
	}

	fn get_header_json_from_rpc_result(
		json_str: &str,
	) -> Result<std::string::String, crate::Error> {
		let v: Value = serde_json::from_str(json_str)?;
		let hjson_str = serde_json::to_string(&v["data"]["header"]["message"])?;
		Ok(hjson_str)
	}

	fn get_attested_header_from_light_client_update_json_str(
		light_client_update_json_str: &str,
	) -> Result<BeaconBlockHeader, crate::Error> {
		let v: Value = serde_json::from_str(light_client_update_json_str)?;
		let attested_header_json_str =
			serde_json::to_string(&v[0]["data"]["attested_header"]["beacon"])?;
		let attested_header: BeaconBlockHeader = serde_json::from_str(&attested_header_json_str)?;

		Ok(attested_header)
	}

	fn get_sync_aggregate_from_light_client_update_json_str(
		light_client_update_json_str: &str,
	) -> Result<SyncAggregate, crate::Error> {
		let v: Value = serde_json::from_str(light_client_update_json_str)?;
		let sync_aggregate_json_str = serde_json::to_string(&v[0]["data"]["sync_aggregate"])?;
		let sync_aggregate: SyncAggregate = serde_json::from_str(&sync_aggregate_json_str)?;

		Ok(sync_aggregate)
	}

	// `signature_slot` is not provided in the current API. The slot is brute-forced
	// until `SyncAggregate` in `BeconBlockBody` in the current slot is equal
	// to `SyncAggregate` in `LightClientUpdate`
	async fn get_signature_slot(
		&self,
		light_client_update_json_str: &str,
	) -> Result<Slot, crate::Error> {
		const CHECK_SLOTS_FORWARD_LIMIT: u64 = 10;

		let v: Value = serde_json::from_str(light_client_update_json_str)?;

		let attested_header_json_str =
			serde_json::to_string(&v[0]["data"]["attested_header"]["beacon"])?;
		let attested_header: BeaconBlockHeader = serde_json::from_str(&attested_header_json_str)?;

		let mut signature_slot = attested_header.slot + 1;

		let sync_aggregate = Self::get_sync_aggregate_from_light_client_update_json_str(
			light_client_update_json_str,
		)?;

		loop {
			if let Ok(beacon_block_body) =
				self.get_beacon_block_body_for_block_id(&format!("{signature_slot}")).await
			{
				if format!(
					"\"{:?}\"",
					beacon_block_body
						.sync_aggregate()
						.map_err(|_| { MissSyncAggregationError })?
						.sync_committee_signature
				) == serde_json::to_string(&sync_aggregate.sync_committee_signature)?
				{
					break
				}
			}

			signature_slot += 1;
			if signature_slot - attested_header.slot > CHECK_SLOTS_FORWARD_LIMIT {
				return Err(SignatureSlotNotFoundError.into())
			}
		}

		Ok(signature_slot)
	}

	async fn get_finality_update_from_light_client_update_json_str(
		&self,
		light_client_update_json_str: &str,
	) -> Result<FinalizedHeaderUpdate, crate::Error> {
		let v: Value = serde_json::from_str(light_client_update_json_str)?;

		let finalized_header_json_str =
			serde_json::to_string(&v[0]["data"]["finalized_header"]["beacon"])?;
		let finalized_header: BeaconBlockHeader = serde_json::from_str(&finalized_header_json_str)?;

		let finalized_branch_json_str = serde_json::to_string(&v[0]["data"]["finality_branch"])?;
		let finalized_branch: Vec<eth_types::H256> =
			serde_json::from_str(&finalized_branch_json_str)?;

		let finalized_block_slot = finalized_header.slot;

		let finalized_block_body = self
			.get_beacon_block_body_for_block_id(&format!("{finalized_block_slot}"))
			.await?;
		let finalized_block_eth1data_proof =
			ExecutionBlockProof::construct_from_beacon_block_body(&finalized_block_body)?;

		Ok(FinalizedHeaderUpdate {
			header_update: HeaderUpdate {
				beacon_header: finalized_header,
				execution_block_hash: eth_types::H256::from(
					finalized_block_eth1data_proof.get_execution_block_hash().0.to_vec(),
				),
				execution_hash_branch: finalized_block_eth1data_proof
					.get_proof()
					.iter()
					.copied()
					.map(|x| eth_types::H256::from(x.0.to_vec()))
					.collect(),
			},
			finality_branch: finalized_branch,
		})
	}

	fn get_sync_committee_update_from_light_client_update_json_str(
		light_client_update_json_str: &str,
	) -> Result<SyncCommitteeUpdate, crate::Error> {
		let v: Value = serde_json::from_str(light_client_update_json_str)?;
		let next_sync_committee_branch_json_str =
			serde_json::to_string(&v[0]["data"]["next_sync_committee_branch"])?;
		let next_sync_committee_branch: Vec<eth_types::H256> =
			serde_json::from_str(&next_sync_committee_branch_json_str)?;

		let next_sync_committee_json_str =
			serde_json::to_string(&v[0]["data"]["next_sync_committee"])?;
		let next_sync_committee: SyncCommittee =
			serde_json::from_str(&next_sync_committee_json_str)?;

		Ok(SyncCommitteeUpdate { next_sync_committee, next_sync_committee_branch })
	}

	pub fn get_period_for_slot(slot: u64) -> u64 {
		slot / (Self::SLOTS_PER_EPOCH * Self::EPOCHS_PER_PERIOD)
	}

	pub async fn get_non_empty_beacon_block_header(
		&self,
		start_slot: u64,
	) -> Result<BeaconBlockHeader, crate::Error> {
		let finalized_slot = self.get_last_finalized_slot_number().await?.as_u64();

		for slot in start_slot..finalized_slot {
			match self.get_beacon_block_header_for_block_id(&format!("{slot}")).await {
				Ok(beacon_block_body) => return Ok(beacon_block_body),
				Err(err) => match err.is_no_block_for_slot_error {
					Some(_) => continue,
					None => return Err(err),
				},
			}
		}

		Err(format!(
			"Unable to get non empty beacon block in range [`{start_slot}`-`{finalized_slot}`)"
		))?
	}

	fn check_block_found_for_slot(&self, json_str: &str) -> Result<(), crate::Error> {
		let parse_json: Value = serde_json::from_str(json_str)?;
		if parse_json.is_object() {
			if let Some(msg_str) = parse_json["message"].as_str() {
				if msg_str.contains("No block found for") {
					return Err(NoBlockForSlotError.into())
				}
			}
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use crate::{
		beacon_rpc_client::BeaconRPCClient,
		config_for_tests::ConfigForTests,
		utils::{read_json_file_from_data_dir, trim_quotes},
	};
	use serde_json::Value;
	use types::{BeaconBlockBody, BeaconBlockHeader, MainnetEthSpec};

	const TIMEOUT_SECONDS: u64 = 30;
	const TIMEOUT_STATE_SECONDS: u64 = 1000;

	fn get_test_config() -> ConfigForTests {
		ConfigForTests::load_from_toml("config_for_tests.toml".try_into().unwrap())
	}

	#[test]
	fn test_get_header_from_json() {
		let beacon_block_header_json_str = r#"
        {
            "slot": "0",
            "proposer_index": "1",
            "parent_root": "0x1cfedbc04788917c188bdab08bf1ed4ece4f352782b61989e142a211fe876c4c",
            "state_root": "0xc40e5fae29997182dbafa0e091d41b27d9bbd6ac388df271e9224d3c0240017f",
            "body_root": "0xb4d27c714e935a2970033c00ebb1d756336ded865e84fd22bec3395971158ab6"
        }
        "#;

		let beacon_block_header: BeaconBlockHeader =
			serde_json::from_str(beacon_block_header_json_str).unwrap();

		assert_eq!(beacon_block_header.slot, 0);
		assert_eq!(beacon_block_header.proposer_index, 1);
		assert_eq!(
			format!("{:?}", beacon_block_header.body_root),
			"0xb4d27c714e935a2970033c00ebb1d756336ded865e84fd22bec3395971158ab6"
		);
		assert_eq!(
			format!("{:?}", beacon_block_header.parent_root),
			"0x1cfedbc04788917c188bdab08bf1ed4ece4f352782b61989e142a211fe876c4c"
		);
		assert_eq!(
			format!("{:?}", beacon_block_header.state_root),
			"0xc40e5fae29997182dbafa0e091d41b27d9bbd6ac388df271e9224d3c0240017f"
		);
	}

	#[test]
	fn test_get_beacon_body_from_json() {
		let beacon_block_body_json_str =
			read_json_file_from_data_dir("beacon_block_body_kiln_slot_741888.json");
		let beacon_block_body: BeaconBlockBody<MainnetEthSpec> =
			serde_json::from_str(&beacon_block_body_json_str).unwrap();

		assert_eq!(
			format!("{:?}", beacon_block_body.eth1_data().deposit_root),
			"0x4b2bfc129d2ce9b4264882bb49c5df18faa8d10b571ee7e87aa85e164da0d2d7"
		);
	}

	#[tokio::test]
	async fn test_get_json_from_raw_request() {
		let config = get_test_config();
		let file_json_str =
			std::fs::read_to_string(&config.path_to_block).expect("Unable to read file");

		let url = format!("{}/eth/v2/beacon/blocks/{}", config.beacon_endpoint, config.first_slot);
		let beacon_rpc_client =
			BeaconRPCClient::new(&url, TIMEOUT_SECONDS, TIMEOUT_STATE_SECONDS, None);
		let rpc_json_str = beacon_rpc_client.get_json_from_raw_request(&url).await;
		assert_eq!(rpc_json_str.unwrap(), file_json_str.trim());
	}

	#[tokio::test]
	async fn test_rpc_beacon_block_body_and_header_smoke() {
		let config = get_test_config();

		let _beacon_block_body = BeaconRPCClient::new(
			&config.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		)
		.get_beacon_block_body_for_block_id(&config.first_slot.to_string())
		.await
		.unwrap();
		let _beacon_block_header = BeaconRPCClient::new(
			&config.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		)
		.get_beacon_block_header_for_block_id(&config.first_slot.to_string())
		.await
		.unwrap();
	}

	#[tokio::test]
	async fn test_get_beacon_block_header() {
		let config = get_test_config();
		let beacon_block_header = BeaconRPCClient::new(
			&config.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		)
		.get_beacon_block_header_for_block_id(&format!("{}", config.first_slot))
		.await
		.unwrap();

		let header_json_str =
			std::fs::read_to_string(config.path_to_header).expect("Unable to read file");
		let v: Value = serde_json::from_str(&header_json_str).unwrap();

		assert_eq!(
			beacon_block_header.slot,
			trim_quotes(v["data"]["header"]["message"]["slot"].to_string())
				.parse::<u64>()
				.unwrap()
		);
		assert_eq!(
			beacon_block_header.proposer_index,
			trim_quotes(v["data"]["header"]["message"]["proposer_index"].to_string())
				.parse::<u64>()
				.unwrap()
		);
		assert_eq!(
			format!("{:?}", beacon_block_header.body_root),
			trim_quotes(v["data"]["header"]["message"]["body_root"].to_string())
		);
		assert_eq!(
			format!("{:?}", beacon_block_header.parent_root),
			trim_quotes(v["data"]["header"]["message"]["parent_root"].to_string())
		);
		assert_eq!(
			format!("{:?}", beacon_block_header.state_root),
			trim_quotes(v["data"]["header"]["message"]["state_root"].to_string())
		);
	}

	#[tokio::test]
	async fn test_get_beacon_block_body() {
		let config = get_test_config();

		let beacon_block_body = BeaconRPCClient::new(
			&config.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		)
		.get_beacon_block_body_for_block_id(&config.first_slot.to_string())
		.await
		.unwrap();

		let block_json_str =
			std::fs::read_to_string(config.path_to_block).expect("Unable to read file");
		let v: Value = serde_json::from_str(&block_json_str).unwrap();
		assert_eq!(
			beacon_block_body.attestations().len(),
			v["data"]["message"]["body"]["attestations"].as_array().unwrap().len()
		);
		assert_eq!(
			format!("{:?}", beacon_block_body.eth1_data().block_hash),
			v["data"]["message"]["body"]["eth1_data"]["block_hash"]
		);
	}

	#[tokio::test]
	async fn test_is_sync() {
		assert!(!BeaconRPCClient::new(
			"https://lodestar-goerli.chainsafe.io",
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None
		)
		.is_syncing()
		.await
		.unwrap());
	}

	#[test]
	fn test_get_header_json_from_rpc_result() {
		let beacon_block_header_response_json =
			read_json_file_from_data_dir("beacon_block_header_response_kiln_slot_741888.json");
		let beacon_block_header_struct_json =
			read_json_file_from_data_dir("beacon_block_header_struct_kiln_slot_741888.json");

		let beacon_header_file: BeaconBlockHeader =
			serde_json::from_str(&beacon_block_header_struct_json).unwrap();
		let beacon_header_rpc: BeaconBlockHeader = serde_json::from_str(
			&BeaconRPCClient::get_header_json_from_rpc_result(&beacon_block_header_response_json)
				.unwrap(),
		)
		.unwrap();

		assert_eq!(beacon_header_file, beacon_header_rpc);
	}

	#[test]
	fn test_beacon_block_body_json_from_rpc_result() {
		let beacon_block_json = read_json_file_from_data_dir("beacon_block_kiln_slot_741888.json");
		let beacon_block_body_json =
			read_json_file_from_data_dir("beacon_block_body_kiln_slot_741888.json");
		let beacon_body_file: BeaconBlockBody<MainnetEthSpec> =
			serde_json::from_str(&beacon_block_body_json).unwrap();
		let beacon_body_rpc: BeaconBlockBody<MainnetEthSpec> = serde_json::from_str(
			&BeaconRPCClient::get_body_json_from_rpc_result(&beacon_block_json).unwrap(),
		)
		.unwrap();

		assert_eq!(beacon_body_file, beacon_body_rpc);
	}

	#[tokio::test]
	async fn test_fetch_light_client_update() {
		let config = get_test_config();

		let beacon_rpc_client = BeaconRPCClient::new(
			&config.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		let file_json_str = std::fs::read_to_string(&config.path_to_light_client_update)
			.expect("Unable to read file");
		let v: Value = serde_json::from_str(&file_json_str).unwrap();
		let period: u64 = BeaconRPCClient::get_period_for_slot(
			v[0]["data"]["attested_header"]["slot"]
				.as_str()
				.unwrap()
				.parse::<u64>()
				.unwrap(),
		);
		println!("period: {period}");
		let light_client_update = beacon_rpc_client.get_light_client_update(period).await.unwrap();

		// check attested_header
		assert_eq!(
			light_client_update.attested_beacon_header.slot,
			v[0]["data"]["attested_header"]["slot"]
				.as_str()
				.unwrap()
				.parse::<u64>()
				.unwrap()
		);
		assert_eq!(
			light_client_update.attested_beacon_header.proposer_index,
			v[0]["data"]["attested_header"]["proposer_index"]
				.as_str()
				.unwrap()
				.parse::<u64>()
				.unwrap()
		);
		assert_eq!(
			serde_json::to_string(&light_client_update.attested_beacon_header.parent_root).unwrap(),
			format!("\"{}\"", v[0]["data"]["attested_header"]["parent_root"].as_str().unwrap())
		);
		assert_eq!(
			serde_json::to_string(&light_client_update.attested_beacon_header.state_root).unwrap(),
			format!("\"{}\"", v[0]["data"]["attested_header"]["state_root"].as_str().unwrap())
		);
		assert_eq!(
			serde_json::to_string(&light_client_update.attested_beacon_header.body_root).unwrap(),
			format!("\"{}\"", v[0]["data"]["attested_header"]["body_root"].as_str().unwrap())
		);

		// check sync_aggregate
		assert_eq!(
			serde_json::to_string(&light_client_update.sync_aggregate.sync_committee_signature)
				.unwrap(),
			format!("{}", v[0]["data"]["sync_aggregate"]["sync_committee_signature"])
		);

		// check signature_slot
		let beacon_block_body = beacon_rpc_client
			.get_beacon_block_body_for_block_id(&format!("{}", light_client_update.signature_slot))
			.await
			.unwrap();
		assert_eq!(
			serde_json::to_string(
				&beacon_block_body.sync_aggregate().unwrap().sync_committee_signature
			)
			.unwrap(),
			format!("{}", v[0]["data"]["sync_aggregate"]["sync_committee_signature"])
		);

		// check finality_update
		let finality_update = light_client_update.finality_update;
		assert_eq!(
			finality_update.header_update.beacon_header.slot,
			v[0]["data"]["finalized_header"]["slot"]
				.as_str()
				.unwrap()
				.parse::<u64>()
				.unwrap()
		);
		assert_eq!(
			serde_json::to_string(&finality_update.header_update.beacon_header.body_root).unwrap(),
			format!("{}", v[0]["data"]["finalized_header"]["body_root"])
		);
		assert_eq!(
			serde_json::to_string(&finality_update.finality_branch[1]).unwrap(),
			format!("{}", v[0]["data"]["finality_branch"][1])
		);

		// check sync_committe_update
		let sync_committe_update = light_client_update.sync_committee_update.unwrap();
		assert_eq!(
			serde_json::to_string(&sync_committe_update.next_sync_committee.aggregate_pubkey)
				.unwrap(),
			format!("{}", v[0]["data"]["next_sync_committee"]["aggregate_pubkey"])
		);
		assert_eq!(
			serde_json::to_string(&sync_committe_update.next_sync_committee_branch[1]).unwrap(),
			format!("{}", v[0]["data"]["next_sync_committee_branch"][1])
		);
	}
}