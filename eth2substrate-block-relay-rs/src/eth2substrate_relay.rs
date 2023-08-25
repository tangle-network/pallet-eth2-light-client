use crate::{
	config::Config,
	prometheus_metrics,
	prometheus_metrics::{
		CHAIN_FINALIZED_EXECUTION_BLOCK_HEIGHT_ON_ETH,
		CHAIN_FINALIZED_EXECUTION_BLOCK_HEIGHT_ON_SUBSTRATE, FAILS_ON_HEADERS_SUBMISSION,
		FAILS_ON_UPDATES_SUBMISSION, LAST_FINALIZED_ETH_SLOT, LAST_FINALIZED_ETH_SLOT_ON_SUBSTRATE,
	},
};

use consensus_types::{
	network_config::{Network, NetworkConfig},
	EPOCHS_PER_SYNC_COMMITTEE_PERIOD, SLOTS_PER_EPOCH,
};
use core::cmp::max;
use eth2_pallet_init::eth_client_pallet_trait::EthClientPalletTrait;
use eth_rpc_client::{
	beacon_rpc_client::BeaconRPCClient, eth1_rpc_client::Eth1RPCClient,
	hand_made_finality_light_client_update::HandMadeFinalityLightClientUpdate,
};
use eth_types::{
	eth2::{Epoch, ForkVersion, LightClientUpdate},
	pallet::ClientMode,
	primitives::FinalExecutionStatus,
	BlockHeader,
};
use log::{debug, info, trace, warn};
use std::{cmp, str::FromStr, thread, time::Duration, vec::Vec};
use tokio::time::sleep;

const ONE_EPOCH_IN_SLOTS: u64 = 32;

macro_rules! skip_fail {
    ($res:expr, $msg:expr, $sleep_time:expr) => {
        match $res {
            Ok(val) => val,
            Err(e) => {
                warn!(target: "relay", "{}. Error: {:?}", $msg, e);
                trace!(target: "relay", "Sleep {} secs before next loop", $sleep_time);
                tokio::time::sleep(Duration::from_secs($sleep_time)).await;
                continue;
            }
        }
    };
}

macro_rules! return_on_fail {
    ($res:expr, $msg:expr) => {
        match $res {
            Ok(val) => val,
            Err(e) => {
                warn!(target: "relay", "{}. Error: {:?}", $msg, e);
                return;
            }
        }
    };
}

macro_rules! return_val_on_fail {
    ($res:expr, $msg:expr, $val:expr) => {
        match $res {
            Ok(val) => val,
            Err(e) => {
                warn!(target: "relay", "{}. Error: {:?}", $msg, e);
                return $val;
            }
        }
    };
}

macro_rules! return_val_on_fail_and_sleep {
    ($res:expr, $msg:expr, $sleep_time:expr, $val:expr) => {
        match $res {
            Ok(val) => val,
            Err(e) => {
                warn!(target: "relay", "{}. Error: {}", $msg, e);
                trace!(target: "relay", "Sleep {} secs before next loop", $sleep_time);
                sleep(Duration::from_secs($sleep_time)).await;
                return $val;
            }
        }
    };
}

pub struct Eth2SubstrateRelay {
	beacon_rpc_client: BeaconRPCClient,
	eth1_rpc_client: Eth1RPCClient,
	eth_client_pallet: Box<dyn EthClientPalletTrait>,
	headers_batch_size: u64,
	bellatrix_fork_epoch: Epoch,
	bellatrix_fork_version: ForkVersion,
	genesis_validators_root: [u8; 32],
	interval_between_light_client_updates_submission_in_epochs: u64,
	max_blocks_for_finalization: u64,
	terminate: bool,
	next_light_client_update: Option<LightClientUpdate>,
	sleep_time_on_sync_secs: u64,
	sleep_time_after_submission_secs: u64,
	get_light_client_update_by_epoch: bool,
}

impl Eth2SubstrateRelay {
	pub async fn init(config: &Config, eth_pallet: Box<dyn EthClientPalletTrait>) -> Self {
		info!(target: "relay", "=== Relay initialization === ");

		let beacon_rpc_client = BeaconRPCClient::new(
			&config.beacon_endpoint,
			config.eth_requests_timeout_seconds,
			config.state_requests_timeout_seconds,
			Some(config.beacon_rpc_version.clone()),
		);

		info!(target: "relay", "=== Beacon RPC Instantiated === ");
		let next_light_client_update =
			Self::get_light_client_update_from_file(config, &beacon_rpc_client)
				.await
				.expect("Error on parsing light client update");

		info!(target: "relay", "=== Next Light Client Update Parsed === ");

		let eth2_network: NetworkConfig =
			NetworkConfig::new(&Network::from_str(&config.ethereum_network.to_string()).unwrap());

		let eth2substrate_relayer = Eth2SubstrateRelay {
			beacon_rpc_client,
			eth1_rpc_client: Eth1RPCClient::new(&config.eth1_endpoint),
			headers_batch_size: config.headers_batch_size as u64,
			interval_between_light_client_updates_submission_in_epochs: config
				.interval_between_light_client_updates_submission_in_epochs,
			max_blocks_for_finalization: config.max_blocks_for_finalization,
			terminate: false,
			next_light_client_update,
			sleep_time_on_sync_secs: config.sleep_time_on_sync_secs,
			sleep_time_after_submission_secs: config.sleep_time_after_submission_secs,
			eth_client_pallet: eth_pallet,
			bellatrix_fork_epoch: eth2_network.bellatrix_fork_epoch,
			bellatrix_fork_version: eth2_network.bellatrix_fork_version,
			genesis_validators_root: eth2_network.genesis_validators_root,
			get_light_client_update_by_epoch: config
				.get_light_client_update_by_epoch
				.unwrap_or(false),
		};

		if let Some(port) = config.prometheus_metrics_port {
			thread::spawn(move || prometheus_metrics::run_prometheus_service(port));
		}

		eth2substrate_relayer
	}

	async fn get_last_finalized_slot_on_substrate(&self) -> anyhow::Result<u64> {
		let last_finalized_slot_on_substrate =
			self.eth_client_pallet.get_finalized_beacon_block_slot().await?;
		LAST_FINALIZED_ETH_SLOT_ON_SUBSTRATE.inc_by(cmp::max(
			0,
			last_finalized_slot_on_substrate as i64 - LAST_FINALIZED_ETH_SLOT_ON_SUBSTRATE.get(),
		));

		if let Ok(last_block_number) = self
			.beacon_rpc_client
			.get_block_number_for_slot(types::Slot::new(last_finalized_slot_on_substrate))
			.await
		{
			CHAIN_FINALIZED_EXECUTION_BLOCK_HEIGHT_ON_SUBSTRATE.inc_by(cmp::max(
				0,
				last_block_number as i64 -
					CHAIN_FINALIZED_EXECUTION_BLOCK_HEIGHT_ON_SUBSTRATE.get(),
			));
		}

		Ok(last_finalized_slot_on_substrate)
	}

	async fn get_last_finalized_slot_on_eth(&self) -> anyhow::Result<u64> {
		let last_finalized_slot_on_eth =
			self.beacon_rpc_client.get_last_finalized_slot_number().await?.as_u64();

		LAST_FINALIZED_ETH_SLOT
			.inc_by(cmp::max(0, last_finalized_slot_on_eth as i64 - LAST_FINALIZED_ETH_SLOT.get()));

		if let Ok(last_block_number) = self
			.beacon_rpc_client
			.get_block_number_for_slot(types::Slot::new(last_finalized_slot_on_eth))
			.await
		{
			CHAIN_FINALIZED_EXECUTION_BLOCK_HEIGHT_ON_ETH.inc_by(cmp::max(
				0,
				last_block_number as i64 - CHAIN_FINALIZED_EXECUTION_BLOCK_HEIGHT_ON_ETH.get(),
			));
		}

		Ok(last_finalized_slot_on_eth)
	}

	pub async fn run(&mut self, max_iterations: Option<u64>) {
		info!(target: "relay", "=== Relay running ===");
		let mut iter_id = 0;
		while !self.terminate {
			iter_id += 1;
			self.set_terminate(iter_id, max_iterations);
			skip_fail!(
				self.wait_for_synchronization().await,
				"Fail to get sync status",
				self.sleep_time_on_sync_secs
			);

			info!(target: "relay", "== New relay loop ==");
			tokio::time::sleep(Duration::from_secs(12)).await;

			let client_mode: ClientMode = skip_fail!(
				self.eth_client_pallet.get_client_mode().await,
				"Fail to get client mode",
				self.sleep_time_on_sync_secs
			);

			let submitted_in_this_iteration = match client_mode {
				ClientMode::SubmitLightClientUpdate => self.submit_light_client_update().await,
				ClientMode::SubmitHeader => self.submit_headers().await,
			};

			if !submitted_in_this_iteration {
				info!(target: "relay", "Sync with ETH network. Sleep {} secs", self.sleep_time_on_sync_secs);
				sleep(Duration::from_secs(self.sleep_time_on_sync_secs)).await;
			}
		}
	}

	async fn submit_light_client_update(&mut self) -> bool {
		info!(target: "relay", "Submit Light Client Update mode");
		self.send_light_client_updates_with_checks().await
	}

	async fn get_max_block_number(&mut self) -> anyhow::Result<u64> {
		if let Some(tail_block_number) =
			self.eth_client_pallet.get_unfinalized_tail_block_number().await?
		{
			Ok(tail_block_number - 1)
		} else {
			self.beacon_rpc_client
				.get_block_number_for_slot(types::Slot::new(
					self.eth_client_pallet.get_finalized_beacon_block_slot().await?,
				))
				.await
		}
	}

	async fn submit_headers(&mut self) -> bool {
		info!(target: "relay", "Submit Headers mode");

		let min_block_number = return_val_on_fail!(
			self.eth_client_pallet.get_last_block_number().await,
			"Failed to get last block number",
			false
		) + 1;

		loop {
			info!(target: "relay", "= Creating headers batch =");

			let current_block_number = return_val_on_fail!(
				self.get_max_block_number().await,
				"Failed to fetch max block number",
				false
			);

			let min_block_number_in_batch =
				max(min_block_number, current_block_number - self.headers_batch_size + 1);
			info!(target: "relay", "Get headers block_number=[{}, {}]", min_block_number_in_batch, current_block_number);

			let mut headers = skip_fail!(
				self.get_execution_blocks_between(current_block_number, min_block_number_in_batch)
					.await,
				"Network problems during fetching execution blocks",
				self.sleep_time_on_sync_secs
			);
			headers.reverse();

			if !self.submit_execution_blocks(headers).await {
				return false
			}

			if min_block_number_in_batch == min_block_number {
				break
			}
		}

		true
	}

	async fn wait_for_synchronization(&self) -> anyhow::Result<()> {
		while self.beacon_rpc_client.is_syncing().await? ||
			self.eth1_rpc_client.is_syncing().await?
		{
			info!(target: "relay", "Waiting for sync...");
			tokio::time::sleep(Duration::from_secs(self.sleep_time_on_sync_secs)).await;
		}
		Ok(())
	}

	async fn get_light_client_update_from_file(
		config: &Config,
		beacon_rpc_client: &BeaconRPCClient,
	) -> anyhow::Result<Option<LightClientUpdate>> {
		let mut next_light_client_update: Option<LightClientUpdate> = None;
		if let Some(path_to_attested_state) = config.clone().path_to_attested_state {
			if config.clone().include_next_sync_committee_to_light_client {
				next_light_client_update = Some(
                    HandMadeFinalityLightClientUpdate::get_light_client_update_from_file_with_next_sync_committee(
                        beacon_rpc_client,
                        &path_to_attested_state,
                    )
					.await
					.expect("Error on getting light client update from file"),
                );
			} else {
				next_light_client_update = Some(
					HandMadeFinalityLightClientUpdate::get_finality_light_client_update_from_file(
						beacon_rpc_client,
						&path_to_attested_state,
					)
					.await
					.expect("Error on getting light client update from file"),
				);
			}
		}
		Ok(next_light_client_update)
	}

	fn set_terminate(&mut self, iter_id: u64, max_iterations: Option<u64>) {
		if let Some(max_iter) = max_iterations {
			if iter_id > max_iter {
				self.terminate = true;
			}
		}
	}

	// Get the BlockHeaders for block number [min_block_number, max_block_number]
	async fn get_execution_blocks_between(
		&self,
		min_block_number: u64,
		max_block_number: u64,
	) -> anyhow::Result<Vec<BlockHeader>> {
		let mut headers: Vec<BlockHeader> = vec![];

		for current_block_number in min_block_number..=max_block_number {
			debug!(target: "relay", "Try add block header for block number={}", current_block_number);
			headers
				.push(self.eth1_rpc_client.get_block_header_by_number(current_block_number).await?);
		}

		Ok(headers)
	}

	async fn submit_execution_blocks(&mut self, headers: Vec<BlockHeader>) -> bool {
		info!(target: "relay", "Try submit headers batch");
		let execution_outcome = return_val_on_fail!(
			self.eth_client_pallet.send_headers(&headers).await,
			"Error on header submission",
			false
		);

		sleep(Duration::from_secs(self.sleep_time_after_submission_secs)).await;

		if let FinalExecutionStatus::Failure = execution_outcome.status {
			FAILS_ON_HEADERS_SUBMISSION.inc();
			// warn!(target: "relay", "FAIL status on Headers submission. Error: {:?}. Transaction URL: https://explorer.{}.near.org/transactions/{}",
			//     error_message, self.substrate_network_name, execution_outcome.transaction.hash);

			false
		} else {
			// info!(target: "relay", "Successful headers submission! Transaction URL: https://explorer.{}.near.org/transactions/{}",
			//                       self.substrate_network_name,
			// execution_outcome.transaction.hash);

			true
		}
	}

	async fn verify_bls_signature_for_finality_update(
		&mut self,
		light_client_update: &LightClientUpdate,
	) -> anyhow::Result<bool> {
		let signature_slot_period =
			BeaconRPCClient::get_period_for_slot(light_client_update.signature_slot);
		let finalized_slot_period = BeaconRPCClient::get_period_for_slot(
			self.eth_client_pallet.get_finalized_beacon_block_slot().await?,
		);

		let light_client_state = self.eth_client_pallet.get_light_client_state().await?;

		let sync_committee = if signature_slot_period == finalized_slot_period {
			light_client_state.current_sync_committee
		} else {
			light_client_state.next_sync_committee
		};

		finality_update_verify::is_correct_finality_update(
			self.bellatrix_fork_epoch,
			self.bellatrix_fork_version,
			self.genesis_validators_root,
			light_client_update,
			sync_committee,
		)
	}
}

// Implementation of functions for submitting light client updates
impl Eth2SubstrateRelay {
	fn is_enough_blocks_for_light_client_update(
		&self,
		last_finalized_slot_on_substrate: u64,
		last_finalized_slot_on_eth: u64,
	) -> bool {
		if (last_finalized_slot_on_eth as i64) - (last_finalized_slot_on_substrate as i64) <
			(ONE_EPOCH_IN_SLOTS * self.interval_between_light_client_updates_submission_in_epochs)
				as i64
		{
			info!(target: "relay", "Light client update were send less then {} epochs ago. Skipping sending light client update", self.interval_between_light_client_updates_submission_in_epochs);
			return false
		}

		if last_finalized_slot_on_eth <= last_finalized_slot_on_substrate {
			info!(target: "relay", "Last finalized slot on Eth equal to last finalized slot on Substrate. Skipping sending light client update.");
			return false
		}

		true
	}

	fn is_shot_run_mode(&self) -> bool {
		self.next_light_client_update.is_some()
	}

	async fn send_light_client_updates_with_checks(&mut self) -> bool {
		let last_finalized_slot_on_substrate: u64 = return_val_on_fail!(
			self.get_last_finalized_slot_on_substrate().await,
			"Error on getting finalized block slot on SUBSTRATE. Skipping sending light client update",
			false
		);

		let last_finalized_slot_on_eth: u64 = return_val_on_fail!(
			self.get_last_finalized_slot_on_eth().await,
            "Error on getting last finalized slot on Ethereum. Skipping sending light client update",
            false
		);

		info!(target: "relay", "last_finalized_slot on substrate/eth {}/{}", last_finalized_slot_on_substrate, last_finalized_slot_on_eth);

		if self.is_enough_blocks_for_light_client_update(
			last_finalized_slot_on_substrate,
			last_finalized_slot_on_eth,
		) {
			self.send_light_client_updates(
				last_finalized_slot_on_substrate,
				last_finalized_slot_on_eth,
			)
			.await;
			return true
		}

		false
	}

	async fn send_light_client_updates(
		&mut self,
		last_finalized_slot_on_near: u64,
		last_finalized_slot_on_eth: u64,
	) {
		info!(target: "relay", "= Sending light client update =");

		if self.is_shot_run_mode() {
			info!(target: "relay", "Try sending light client update from file");
			self.send_light_client_update_from_file().await;
			return
		}

		if self.get_light_client_update_by_epoch &&
			self.send_regular_light_client_update_by_epoch(
				last_finalized_slot_on_eth,
				last_finalized_slot_on_near,
			)
			.await
		{
			return
		}

		if last_finalized_slot_on_eth >=
			last_finalized_slot_on_near + self.max_blocks_for_finalization
		{
			info!(target: "relay", "Too big gap between slot of finalized block on NEAR and ETH. Sending hand made light client update");
			self.send_hand_made_light_client_update(last_finalized_slot_on_near).await;
		} else {
			self.send_regular_light_client_update(
				last_finalized_slot_on_eth,
				last_finalized_slot_on_near,
			)
			.await;
		}
	}

	async fn send_light_client_update_from_file(&mut self) {
		if let Some(light_client_update) = self.next_light_client_update.clone() {
			self.send_specific_light_client_update(light_client_update).await;
			self.terminate = true;
		}
	}

	async fn send_regular_light_client_update(
		&mut self,
		last_finalized_slot_on_eth: u64,
		last_finalized_slot_on_substrate: u64,
	) {
		let last_eth2_period_on_substrate_chain =
			BeaconRPCClient::get_period_for_slot(last_finalized_slot_on_substrate);
		info!(target: "relay", "Last finalized slot/period on substrate={}/{}", last_finalized_slot_on_substrate, last_eth2_period_on_substrate_chain);

		let end_period = BeaconRPCClient::get_period_for_slot(last_finalized_slot_on_eth);
		info!(target: "relay", "Last finalized slot/period on ethereum={}/{}", last_finalized_slot_on_eth, end_period);

		let light_client_update = if end_period == last_eth2_period_on_substrate_chain {
			debug!(target: "relay", "Finalized period on ETH and SUBSTRATE are equal. Don't fetch sync commity update");
			return_on_fail!(
				self.beacon_rpc_client.get_finality_light_client_update().await,
				"Error on getting light client update. Skipping sending light client update"
			)
		} else {
			debug!(target: "relay", "Finalized period on ETH and SUBSTRATE are different. Fetching sync commity update");
			return_on_fail!(
				self.beacon_rpc_client
					.get_light_client_update(last_eth2_period_on_substrate_chain + 1)
					.await,
				"Error on getting light client update. Skipping sending light client update"
			)
		};

		self.send_specific_light_client_update(light_client_update).await;
	}

	async fn send_regular_light_client_update_by_epoch(
		&mut self,
		last_finalized_slot_on_eth: u64,
		last_finalized_slot_on_substrate: u64,
	) -> bool {
		let last_eth2_period_on_substrate_chain =
			BeaconRPCClient::get_period_for_slot(last_finalized_slot_on_substrate);
		info!(target: "relay", "Last finalized slot/period on substrate={}/{}", last_finalized_slot_on_substrate, last_eth2_period_on_substrate_chain);

		let end_period = BeaconRPCClient::get_period_for_slot(last_finalized_slot_on_eth);
		info!(target: "relay", "Last finalized slot/period on ethereum={}/{}", last_finalized_slot_on_eth, end_period);

		let last_epoch = last_finalized_slot_on_substrate / SLOTS_PER_EPOCH;
		let last_period = last_epoch / EPOCHS_PER_SYNC_COMMITTEE_PERIOD;
		let mut update_epoch =
			last_epoch + self.interval_between_light_client_updates_submission_in_epochs + 2;

		let light_client_update = loop {
			let res = self.beacon_rpc_client.get_light_client_update_by_epoch(update_epoch).await;

			if let Ok(res) = res {
				let update_epoch =
					res.finality_update.header_update.beacon_header.slot / SLOTS_PER_EPOCH;
				let update_period = update_epoch / EPOCHS_PER_SYNC_COMMITTEE_PERIOD;

				if update_period > last_period + 1 {
					debug!(target: "relay", "Finalized period on ETH and SUBSTRATE are different. Fetching sync commity update");
					let res = return_val_on_fail!(
                        self.beacon_rpc_client
                            .get_light_client_update(update_period)
							.await,
                        "Error on getting light client update. Skipping sending light client update", false
                    );

					break res
				}

				break res
			}

			warn!(target: "relay", "Error: {}", res.unwrap_err());
			sleep(Duration::from_secs(5)).await;

			update_epoch -= 1;
		};

		self.send_specific_light_client_update(light_client_update).await
	}

	async fn get_attested_slot(
		&mut self,
		last_finalized_slot_on_substrate: u64,
	) -> anyhow::Result<u64> {
		const EXPECTED_EPOCHS_BETWEEN_HEAD_AND_FINALIZED_BLOCKS: u64 = 2;
		let next_finalized_slot = last_finalized_slot_on_substrate +
			self.interval_between_light_client_updates_submission_in_epochs * ONE_EPOCH_IN_SLOTS;
		let attested_slot = next_finalized_slot +
			EXPECTED_EPOCHS_BETWEEN_HEAD_AND_FINALIZED_BLOCKS * ONE_EPOCH_IN_SLOTS;

		let attested_slot: u64 = self
			.beacon_rpc_client
			.get_non_empty_beacon_block_header(attested_slot)
			.await?
			.slot
			.into();
		trace!(target: "relay", "Chosen attested slot {}", attested_slot);

		Ok(attested_slot)
	}

	async fn send_hand_made_light_client_update(&mut self, last_finalized_slot_on_substrate: u64) {
		let mut attested_slot = return_on_fail!(
			self.get_attested_slot(last_finalized_slot_on_substrate).await,
			"Error on getting attested slot"
		);

		let include_next_sync_committee =
			BeaconRPCClient::get_period_for_slot(last_finalized_slot_on_substrate) !=
				BeaconRPCClient::get_period_for_slot(attested_slot);

		loop {
			let light_client_update = return_on_fail!(
				HandMadeFinalityLightClientUpdate::get_finality_light_client_update(
					&self.beacon_rpc_client,
					attested_slot,
					include_next_sync_committee,
				)
				.await,
				format!(
					"Error on getting hand made light client update for attested slot={attested_slot}."
				)
			);

			let finality_update_slot =
				light_client_update.finality_update.header_update.beacon_header.slot;

			if finality_update_slot <= last_finalized_slot_on_substrate {
				info!(target: "relay", "Finality update slot for hand made light client update <= last finality update on SUBSTRATE. Increment gap for attested slot and skipping light client update.");
				attested_slot = return_on_fail!(
					self.get_attested_slot(last_finalized_slot_on_substrate + ONE_EPOCH_IN_SLOTS)
						.await,
					"Error on getting attested slot"
				);
				continue
			}

			trace!(target: "relay", "Hand made light client update: {:?}", light_client_update);
			self.send_specific_light_client_update(light_client_update).await;
			return
		}
	}

	async fn send_specific_light_client_update(
		&mut self,
		light_client_update: LightClientUpdate,
	) -> bool {
		let verification_result = return_val_on_fail!(
			self.verify_bls_signature_for_finality_update(&light_client_update).await,
			"Error on bls verification. Skip sending the light client update",
			false
		);

		if verification_result {
			info!(target: "relay", "PASS bls signature verification!");
		} else {
			warn!(target: "relay", "NOT PASS bls signature verification. Skip sending this light client update");
			return false
		}

		let execution_outcome = return_val_on_fail_and_sleep!(
			self.eth_client_pallet
				.send_light_client_update(light_client_update.clone())
				.await,
			"Fail to send light client update",
			self.sleep_time_on_sync_secs,
			false
		);

		info!(target: "relay", "Sending light client update");

		if let FinalExecutionStatus::Failure = execution_outcome.status {
			FAILS_ON_UPDATES_SUBMISSION.inc();
			warn!(target: "relay", "FAIL status on Light Client Update submission.");
		}

		// info!(target: "relay", "Successful light client update submission! Transaction URL: https://explorer.{}.near.org/transactions/{}",
		// 						self.substrate_network_name, execution_outcome.transaction.hash);

		let finalized_block_number = return_val_on_fail!(
			self.beacon_rpc_client
				.get_block_number_for_slot(types::Slot::new(
					light_client_update.finality_update.header_update.beacon_header.slot
				))
				.await,
			"Fail on getting finalized block number",
			false
		);

		info!(target: "relay", "Finalized block number from light client update = {}", finalized_block_number);
		sleep(Duration::from_secs(self.sleep_time_after_submission_secs)).await;
		true
	}
}
