use eth2_pallet_init::eth_client_pallet_trait::EthClientPalletTrait;
use eth_rpc_client::{beacon_rpc_client::BeaconRPCClient, errors::ExecutionPayloadError};
use eth_types::H256;
use log::{info, trace};
use std::cmp;

pub struct LastSlotSearcher {
	enable_binsearch: bool,
}

type EthClientContract = Box<dyn EthClientPalletTrait>;

// Implementation of functions for searching last slot on SUBSTRATE contract
impl LastSlotSearcher {
	pub fn new(enable_binsearch: bool) -> Self {
		Self { enable_binsearch }
	}

	pub async fn get_last_slot(
		&mut self,
		last_eth_slot: u64,
		beacon_rpc_client: &BeaconRPCClient,
		eth_client_contract: &EthClientContract,
	) -> Result<u64, crate::Error> {
		info!(target: "relay", "= Search for last slot on SUBSTRATE =");

		let finalized_slot =
			eth_client_contract.get_finalized_beacon_block_slot().await.map_err(to_error)?;
		let finalized_number = beacon_rpc_client.get_block_number_for_slot(finalized_slot).await?;
		info!(target: "relay", "Finalized slot/block_number on SUBSTRATE={}/{}", finalized_slot, finalized_number);

		let last_submitted_slot = eth_client_contract.get_last_submitted_slot().await?;
		trace!(target: "relay", "Last submitted slot={}", last_submitted_slot);

		let slot = cmp::max(finalized_slot, last_submitted_slot);
		trace!(target: "relay", "Init slot for search as {}", slot);

		if self.enable_binsearch {
			self.binary_slot_search(
				slot,
				finalized_slot,
				last_eth_slot,
				beacon_rpc_client,
				eth_client_contract,
			)
			.await
		} else {
			self.linear_slot_search(
				slot,
				finalized_slot,
				last_eth_slot,
				beacon_rpc_client,
				eth_client_contract,
			)
			.await
		}
	}

	// Search for the slot before the first unknown slot on SUBSTRATE
	// Assumptions:
	//     (1) start_slot is known on SUBSTRATE
	//     (2) last_slot is unknown on SUBSTRATE
	// Return error in case of problem with network connection
	async fn binary_slot_search(
		&self,
		slot: u64,
		finalized_slot: u64,
		last_eth_slot: u64,
		beacon_rpc_client: &BeaconRPCClient,
		eth_client_contract: &EthClientContract,
	) -> Result<u64, crate::Error> {
		if slot == finalized_slot {
			return self
				.binsearch_slot_forward(
					slot,
					last_eth_slot + 1,
					beacon_rpc_client,
					eth_client_contract,
				)
				.await
		}

		match self
			.block_known_on_substrate(slot, beacon_rpc_client, eth_client_contract)
			.await
		{
			Ok(true) =>
				self.binsearch_slot_forward(
					slot,
					last_eth_slot + 1,
					beacon_rpc_client,
					eth_client_contract,
				)
				.await,
			Ok(false) =>
				self.binsearch_slot_range(
					finalized_slot,
					slot,
					beacon_rpc_client,
					eth_client_contract,
				)
				.await,
			Err(err) => match err.is_no_block_for_slot_error {
				Some(_) => {
					let (left_slot, slot_on_substrate) = self
						.find_left_non_error_slot(
							slot + 1,
							last_eth_slot + 1,
							1,
							beacon_rpc_client,
							eth_client_contract,
						)
						.await;
					match slot_on_substrate {
						true =>
							self.binsearch_slot_forward(
								left_slot,
								last_eth_slot + 1,
								beacon_rpc_client,
								eth_client_contract,
							)
							.await,
						false =>
							self.binsearch_slot_range(
								finalized_slot,
								slot,
								beacon_rpc_client,
								eth_client_contract,
							)
							.await,
					}
				},
				None => Err(err),
			},
		}
	}

	// Search for the slot before the first unknown slot on SUBSTRATE
	// Assumptions:
	// (1) start_slot is known on SUBSTRATE
	// (2) last_slot is unknown on SUBSTRATE
	// Return error in case of problem with network connection
	async fn binsearch_slot_forward(
		&self,
		slot: u64,
		max_slot: u64,
		beacon_rpc_client: &BeaconRPCClient,
		eth_client_contract: &EthClientContract,
	) -> Result<u64, crate::Error> {
		let mut current_step = 1;
		let mut prev_slot = slot;
		while slot + current_step < max_slot {
			match self
				.block_known_on_substrate(
					slot + current_step,
					beacon_rpc_client,
					eth_client_contract,
				)
				.await
			{
				Ok(true) => {
					prev_slot = slot + current_step;
					current_step = cmp::min(current_step * 2, max_slot - slot);
				},
				Ok(false) => break,
				Err(err) => match err.is_no_block_for_slot_error {
					Some(_) => {
						let (slot_id, slot_on_substrate) = self
							.find_left_non_error_slot(
								slot + current_step - 1,
								prev_slot,
								-1,
								beacon_rpc_client,
								eth_client_contract,
							)
							.await;
						if slot_on_substrate {
							prev_slot = slot_id;
							current_step = cmp::min(current_step * 2, max_slot - slot);
						} else {
							current_step = slot_id - slot;
							break
						}
					},
					None => return Err(err),
				},
			}
		}

		self.binsearch_slot_range(
			prev_slot,
			slot + current_step,
			beacon_rpc_client,
			eth_client_contract,
		)
		.await
	}

	// Search for the slot before the first unknown slot on SUBSTRATE
	// Assumptions:
	// (1) start_slot is known on SUBSTRATE
	// (2) last_slot is unknown on SUBSTRATE
	// Return error in case of problem with network connection
	async fn binsearch_slot_range(
		&self,
		start_slot: u64,
		last_slot: u64,
		beacon_rpc_client: &BeaconRPCClient,
		eth_client_contract: &EthClientContract,
	) -> Result<u64, crate::Error> {
		let mut start_slot = start_slot;
		let mut last_slot = last_slot;
		while start_slot + 1 < last_slot {
			let mid_slot = start_slot + (last_slot - start_slot) / 2;
			match self
				.block_known_on_substrate(mid_slot, beacon_rpc_client, eth_client_contract)
				.await
			{
				Ok(true) => start_slot = mid_slot,
				Ok(false) => last_slot = mid_slot,
				Err(err) => match err.is_no_block_for_slot_error {
					Some(_) => {
						let (left_slot, is_left_slot_on_substrate) = self
							.find_left_non_error_slot(
								mid_slot - 1,
								start_slot,
								-1,
								beacon_rpc_client,
								eth_client_contract,
							)
							.await;
						if is_left_slot_on_substrate {
							start_slot = mid_slot;
						} else {
							last_slot = left_slot;
						}
					},
					None => return Err(err),
				},
			}
		}

		Ok(start_slot)
	}

	// Returns the last slot known with block known on SUBSTRATE
	// Slot -- expected last known slot
	// finalized_slot -- last finalized slot on SUBSTRATE, assume as known slot
	// last_eth_slot -- head slot on Eth
	async fn linear_slot_search(
		&self,
		slot: u64,
		finalized_slot: u64,
		last_eth_slot: u64,
		beacon_rpc_client: &BeaconRPCClient,
		eth_client_contract: &EthClientContract,
	) -> Result<u64, crate::Error> {
		if slot == finalized_slot {
			return self
				.linear_search_forward(slot, last_eth_slot, beacon_rpc_client, eth_client_contract)
				.await
		}

		match self
			.block_known_on_substrate(slot, beacon_rpc_client, eth_client_contract)
			.await
		{
			Ok(true) =>
				self.linear_search_forward(
					slot,
					last_eth_slot,
					beacon_rpc_client,
					eth_client_contract,
				)
				.await,
			Ok(false) =>
				self.linear_search_backward(
					finalized_slot,
					slot,
					beacon_rpc_client,
					eth_client_contract,
				)
				.await,
			Err(err) => match err.is_no_block_for_slot_error {
				Some(_) => {
					let (left_slot, slot_on_substrate) = self
						.find_left_non_error_slot(
							slot + 1,
							last_eth_slot + 1,
							1,
							beacon_rpc_client,
							eth_client_contract,
						)
						.await;

					match slot_on_substrate {
						true =>
							self.linear_search_forward(
								left_slot,
								last_eth_slot,
								beacon_rpc_client,
								eth_client_contract,
							)
							.await,
						false =>
							self.linear_search_backward(
								finalized_slot,
								left_slot - 1,
								beacon_rpc_client,
								eth_client_contract,
							)
							.await,
					}
				},
				None => Err(err),
			},
		}
	}

	// Returns the slot before the first unknown block on SUBSTRATE
	// The search range is [slot .. max_slot)
	// If there is no unknown block in this range max_slot - 1 will be returned
	// Assumptions:
	//     (1) block for slot is submitted to Substrate
	//     (2) block for max_slot is not submitted to Substrate
	async fn linear_search_forward(
		&self,
		slot: u64,
		max_slot: u64,
		beacon_rpc_client: &BeaconRPCClient,
		eth_client_contract: &EthClientContract,
	) -> Result<u64, crate::Error> {
		let mut slot = slot;
		while slot < max_slot {
			match self
				.block_known_on_substrate(slot + 1, beacon_rpc_client, eth_client_contract)
				.await
			{
				Ok(true) => slot += 1,
				Ok(false) => break,
				Err(err) => match err.is_no_block_for_slot_error {
					Some(_) => slot += 1,
					None => return Err(err),
				},
			}
		}

		Ok(slot)
	}

	// Returns the slot before the first unknown block on SUBSTRATE
	// The search range is [last_slot .. start_slot)
	// If no such block are found the start_slot will be returned
	// Assumptions:
	//     (1) block for start_slot is submitted to Substrate
	//     (2) block for last_slot + 1 is not submitted to Substrate
	async fn linear_search_backward(
		&self,
		start_slot: u64,
		last_slot: u64,
		beacon_rpc_client: &BeaconRPCClient,
		eth_client_contract: &EthClientContract,
	) -> Result<u64, crate::Error> {
		let mut slot = last_slot;
		let mut last_false_slot = slot + 1;

		while slot > start_slot {
			match self
				.block_known_on_substrate(slot, beacon_rpc_client, eth_client_contract)
				.await
			{
				Ok(true) => break,
				Ok(false) => {
					last_false_slot = slot;
					slot -= 1
				},
				Err(err) => match err.is_no_block_for_slot_error {
					Some(_) => slot -= 1,
					None => return Err(err),
				},
			}
		}

		Ok(last_false_slot - 1)
	}

	// Find the leftmost non-empty slot. Search range: [left_slot, right_slot).
	// Returns pair: (1) slot_id and (2) is this block already known on Eth client on SUBSTRATE
	// Assume that right_slot is non-empty and it's block were submitted to Substrate,
	// so if non correspondent block is found we return (right_slot, false)
	async fn find_left_non_error_slot(
		&self,
		left_slot: u64,
		right_slot: u64,
		step: i8,
		beacon_rpc_client: &BeaconRPCClient,
		eth_client_contract: &EthClientContract,
	) -> (u64, bool) {
		trace!(target: "relay", " left non error slor {}, {}, {}", left_slot, right_slot, step);

		let mut slot = left_slot;
		while slot != right_slot {
			match self
				.block_known_on_substrate(slot, beacon_rpc_client, eth_client_contract)
				.await
			{
				Ok(v) => return (slot, v),
				Err(_) =>
					if step > 0 {
						slot += 1;
					} else {
						slot -= 1;
					},
			};
		}

		if step > 0 {
			(slot, false)
		} else {
			(slot, true)
		}
	}

	// Check if the block for current slot in Eth2 already were submitted to Substrate
	// Returns Error if slot doesn't contain any block
	async fn block_known_on_substrate(
		&self,
		slot: u64,
		beacon_rpc_client: &BeaconRPCClient,
		eth_client_contract: &EthClientContract,
	) -> Result<bool, crate::Error> {
		trace!(target: "relay", "Check if block with slot={} on SUBSTRATE", slot);
		match beacon_rpc_client.get_beacon_block_body_for_block_id(&format!("{slot}")).await {
			Ok(beacon_block_body) => {
				let hash: H256 = H256::from(
					beacon_block_body
						.execution_payload()
						.map_err(|_| ExecutionPayloadError)?
						.execution_payload
						.block_hash
						.into_root()
						.as_bytes(),
				);

				if eth_client_contract.is_known_block(&hash).await.map_err(to_error)? {
					trace!(target: "relay", "Block with slot={} was found on SUBSTRATE", slot);
					Ok(true)
				} else {
					trace!(target: "relay", "Block with slot={} not found on SUBSTRATE", slot);
					Ok(false)
				}
			},
			Err(err) => {
				trace!(target: "relay", "Error \"{:?}\" in getting beacon block body for slot={}", err, slot);
				Err(err)?
			},
		}
	}
}

#[cfg(test)]
mod tests {
	use crate::{
		config_for_tests::ConfigForTests, last_slot_searcher::LastSlotSearcher,
		test_utils::get_client_pallet,
	};
	use eth2_pallet_init::eth_client_pallet_trait::EthClientPalletTrait;
	use eth_rpc_client::{beacon_rpc_client::BeaconRPCClient, eth1_rpc_client::Eth1RPCClient};
	use eth_types::BlockHeader;

	const TIMEOUT_SECONDS: u64 = 30;
	const TIMEOUT_STATE_SECONDS: u64 = 1000;

	fn get_test_config() -> ConfigForTests {
		ConfigForTests::load_from_toml("config_for_tests.toml".try_into().unwrap())
	}

	async fn get_execution_block_by_slot(
		slot: u64,
		beacon_rpc_client: &BeaconRPCClient,
		eth1_rpc_client: &Eth1RPCClient,
	) -> Result<BlockHeader, crate::Error> {
		match beacon_rpc_client.get_block_number_for_slot(slot).await {
			Ok(block_number) => eth1_rpc_client.get_block_header_by_number(block_number).await,
			Err(err) => Err(err),
		}
	}

	async fn send_execution_blocks(
		beacon_rpc_client: &BeaconRPCClient,
		eth_client_contract: &mut Box<dyn EthClientPalletTrait>,
		eth1_rpc_client: &Eth1RPCClient,
		start_slot: u64,
		end_slot: u64,
	) {
		let mut slot = start_slot;
		let mut blocks: Vec<BlockHeader> = vec![];
		while slot <= end_slot {
			if let Ok(block) =
				get_execution_block_by_slot(slot, beacon_rpc_client, eth1_rpc_client).await
			{
				blocks.push(block)
			}
			slot += 1;
		}

		eth_client_contract.send_headers(&blocks, end_slot).await.unwrap();
	}

	#[tokio::test]
	async fn test_block_known_on_substrate() {
		let config_for_test = get_test_config();

		let mut eth_client_contract = get_client_pallet(true, &config_for_test).await;
		eth_client_contract.register_submitter().await.unwrap();
		let beacon_rpc_client = BeaconRPCClient::new(
			&config_for_test.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		let eth1_rpc_client = Eth1RPCClient::new(&config_for_test.eth1_endpoint);
		let last_slot_searcher = LastSlotSearcher::new(true);

		let is_block_known = last_slot_searcher
			.block_known_on_substrate(
				config_for_test.slot_without_block,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await;
		if is_block_known.is_ok() {
			panic!();
		}

		let is_block_known = last_slot_searcher
			.block_known_on_substrate(
				config_for_test.first_slot,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await;

		match is_block_known {
			Ok(is_block_known) => assert!(!is_block_known),
			Err(_) => panic!(),
		}

		let finalized_slot = eth_client_contract.get_finalized_beacon_block_slot().await.unwrap();

		send_execution_blocks(
			&beacon_rpc_client,
			&mut eth_client_contract,
			&eth1_rpc_client,
			finalized_slot + 1,
			finalized_slot + 1,
		)
		.await;

		let is_block_known = last_slot_searcher
			.block_known_on_substrate(finalized_slot + 1, &beacon_rpc_client, &eth_client_contract)
			.await;
		match is_block_known {
			Ok(is_block_known) => assert!(is_block_known),
			Err(_) => panic!(),
		}
	}

	#[tokio::test]
	async fn test_find_left_non_error_slot() {
		let config_for_test = get_test_config();
		let mut eth_client_contract = get_client_pallet(true, &config_for_test).await;
		eth_client_contract.register_submitter().await.unwrap();
		let beacon_rpc_client = BeaconRPCClient::new(
			&config_for_test.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		let eth1_rpc_client = Eth1RPCClient::new(&config_for_test.eth1_endpoint);
		let last_slot_searcher = LastSlotSearcher::new(true);

		let (left_non_empty_slot, is_known_block) = last_slot_searcher
			.find_left_non_error_slot(
				config_for_test.left_empty_slot - 1,
				config_for_test.right_empty_slot + 2,
				1,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await;
		assert_eq!(left_non_empty_slot, config_for_test.left_empty_slot - 1);
		assert!(!is_known_block);

		let (left_non_empty_slot, is_known_block) = last_slot_searcher
			.find_left_non_error_slot(
				config_for_test.left_empty_slot,
				config_for_test.right_empty_slot + 2,
				1,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await;
		assert_eq!(left_non_empty_slot, config_for_test.right_empty_slot + 1);
		assert!(!is_known_block);

		let (left_non_empty_slot, is_known_block) = last_slot_searcher
			.find_left_non_error_slot(
				config_for_test.left_empty_slot,
				config_for_test.right_empty_slot,
				1,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await;
		assert_eq!(left_non_empty_slot, config_for_test.right_empty_slot);
		assert!(!is_known_block);

		let (left_non_empty_slot, is_known_block) = last_slot_searcher
			.find_left_non_error_slot(
				config_for_test.right_empty_slot,
				config_for_test.right_empty_slot + 2,
				1,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await;
		assert_eq!(left_non_empty_slot, config_for_test.right_empty_slot + 1);
		assert!(!is_known_block);

		let finalized_slot = eth_client_contract.get_finalized_beacon_block_slot().await.unwrap();

		send_execution_blocks(
			&beacon_rpc_client,
			&mut eth_client_contract,
			&eth1_rpc_client,
			finalized_slot + 1,
			finalized_slot + 1,
		)
		.await;

		let (left_non_empty_slot, is_known_block) = last_slot_searcher
			.find_left_non_error_slot(
				finalized_slot + 1,
				finalized_slot + 2,
				1,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await;
		assert_eq!(left_non_empty_slot, finalized_slot + 1);
		assert!(is_known_block);
	}

	#[tokio::test]
	async fn test_linear_search_backward() {
		let config_for_test = get_test_config();
		let mut eth_client_contract = get_client_pallet(true, &config_for_test).await;
		eth_client_contract.register_submitter().await.unwrap();
		let beacon_rpc_client = BeaconRPCClient::new(
			&config_for_test.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		let eth1_rpc_client = Eth1RPCClient::new(&config_for_test.eth1_endpoint);
		let last_slot_searcher = LastSlotSearcher::new(true);

		let finalized_slot = eth_client_contract.get_finalized_beacon_block_slot().await.unwrap();
		send_execution_blocks(
			&beacon_rpc_client,
			&mut eth_client_contract,
			&eth1_rpc_client,
			finalized_slot + 1,
			finalized_slot + 2,
		)
		.await;

		let last_submitted_block = last_slot_searcher
			.linear_search_backward(
				finalized_slot + 1,
				finalized_slot + 10,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_submitted_block, finalized_slot + 2);

		send_execution_blocks(
			&beacon_rpc_client,
			&mut eth_client_contract,
			&eth1_rpc_client,
			finalized_slot + 3,
			config_for_test.slot_without_block - 1,
		)
		.await;

		let last_submitted_block = last_slot_searcher
			.linear_search_backward(
				finalized_slot + 1,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_submitted_block, config_for_test.slot_without_block);
	}

	#[tokio::test]
	async fn test_linear_search_forward() {
		let config_for_test = get_test_config();
		let mut eth_client_contract = get_client_pallet(true, &config_for_test).await;
		eth_client_contract.register_submitter().await.unwrap();
		let beacon_rpc_client = BeaconRPCClient::new(
			&config_for_test.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		let eth1_rpc_client = Eth1RPCClient::new(&config_for_test.eth1_endpoint);
		let last_slot_searcher = LastSlotSearcher::new(true);

		let mut slot = eth_client_contract.get_finalized_beacon_block_slot().await.unwrap();
		slot += 1;

		send_execution_blocks(
			&beacon_rpc_client,
			&mut eth_client_contract,
			&eth1_rpc_client,
			slot,
			config_for_test.slot_without_block - 2,
		)
		.await;

		let last_block_on_substrate = last_slot_searcher
			.linear_search_forward(
				eth_client_contract.get_finalized_beacon_block_slot().await.unwrap() + 1,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();

		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block - 2);

		send_execution_blocks(
			&beacon_rpc_client,
			&mut eth_client_contract,
			&eth1_rpc_client,
			config_for_test.slot_without_block - 1,
			config_for_test.slot_without_block - 1,
		)
		.await;

		let last_block_on_substrate = last_slot_searcher
			.linear_search_forward(
				eth_client_contract.get_finalized_beacon_block_slot().await.unwrap() + 1,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();

		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block);
	}

	#[tokio::test]
	async fn test_linear_slot_search() {
		let config_for_test = get_test_config();
		let mut eth_client_contract = get_client_pallet(true, &config_for_test).await;
		eth_client_contract.register_submitter().await.unwrap();
		let beacon_rpc_client = BeaconRPCClient::new(
			&config_for_test.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		let eth1_rpc_client = Eth1RPCClient::new(&config_for_test.eth1_endpoint);
		let last_slot_searcher = LastSlotSearcher::new(true);

		let mut slot = eth_client_contract.get_finalized_beacon_block_slot().await.unwrap();
		slot += 1;
		send_execution_blocks(
			&beacon_rpc_client,
			&mut eth_client_contract,
			&eth1_rpc_client,
			slot,
			config_for_test.slot_without_block - 1,
		)
		.await;

		let finalized_slot = eth_client_contract.get_finalized_beacon_block_slot().await.unwrap();

		let last_block_on_substrate = last_slot_searcher
			.linear_slot_search(
				config_for_test.slot_without_block - 1,
				finalized_slot,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block);

		let last_block_on_substrate = last_slot_searcher
			.linear_slot_search(
				config_for_test.slot_without_block,
				finalized_slot,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block);

		let last_block_on_substrate = last_slot_searcher
			.linear_slot_search(
				config_for_test.first_slot + 1,
				finalized_slot,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block);

		let last_block_on_substrate = last_slot_searcher
			.linear_slot_search(
				config_for_test.slot_without_block + 5,
				finalized_slot,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block);
	}

	#[tokio::test]
	#[should_panic]
	async fn test_error_on_connection_problem() {
		let config_for_test = get_test_config();
		let mut eth_client_contract = get_client_pallet(true, &config_for_test).await;
		eth_client_contract.register_submitter().await.unwrap();
		let mut beacon_rpc_client = BeaconRPCClient::new(
			&config_for_test.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		let eth1_rpc_client = Eth1RPCClient::new(&config_for_test.eth1_endpoint);
		let last_slot_searcher = LastSlotSearcher::new(true);

		let finalized_slot = eth_client_contract.get_finalized_beacon_block_slot().await.unwrap();

		send_execution_blocks(
			&beacon_rpc_client,
			&mut eth_client_contract,
			&eth1_rpc_client,
			finalized_slot + 1,
			finalized_slot + 2,
		)
		.await;

		beacon_rpc_client = BeaconRPCClient::new(
			"http://httpstat.us/504/",
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		last_slot_searcher
			.linear_slot_search(
				finalized_slot + 1,
				finalized_slot,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
	}

	#[tokio::test]
	async fn test_binsearch_slot_range() {
		let config_for_test = get_test_config();
		let mut eth_client_contract = get_client_pallet(true, &config_for_test).await;
		eth_client_contract.register_submitter().await.unwrap();
		let mut beacon_rpc_client = BeaconRPCClient::new(
			&config_for_test.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		let eth1_rpc_client = Eth1RPCClient::new(&config_for_test.eth1_endpoint);
		let last_slot_searcher = LastSlotSearcher::new(true);

		let finalized_beacon_slot =
			eth_client_contract.get_finalized_beacon_block_slot().await.unwrap();

		send_execution_blocks(
			&beacon_rpc_client,
			&mut eth_client_contract,
			&eth1_rpc_client,
			finalized_beacon_slot + 1,
			config_for_test.slot_without_block - 2,
		)
		.await;

		let last_block_on_substrate = last_slot_searcher
			.binsearch_slot_range(
				eth_client_contract.get_finalized_beacon_block_slot().await.unwrap() + 1,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block - 2);

		send_execution_blocks(
			&beacon_rpc_client,
			&mut eth_client_contract,
			&eth1_rpc_client,
			config_for_test.slot_without_block - 1,
			config_for_test.slot_without_block - 1,
		)
		.await;
		let last_block_on_substrate = last_slot_searcher
			.binsearch_slot_range(
				eth_client_contract.get_finalized_beacon_block_slot().await.unwrap() + 1,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block);

		let last_block_on_substrate = last_slot_searcher
			.binsearch_slot_range(
				eth_client_contract.get_finalized_beacon_block_slot().await.unwrap() + 1,
				config_for_test.slot_without_block,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block - 1);

		let last_block_on_substrate = last_slot_searcher
			.binsearch_slot_range(
				config_for_test.slot_without_block,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block);

		beacon_rpc_client = BeaconRPCClient::new(
			"http://httpstat.us/504/",
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		if last_slot_searcher
			.binsearch_slot_range(
				eth_client_contract.get_finalized_beacon_block_slot().await.unwrap() + 1,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.is_ok()
		{
			panic!("binarysearch returns result in unworking network");
		}
	}

	#[tokio::test]
	async fn test_binsearch_slot_forward() {
		let config_for_test = get_test_config();
		let mut eth_client_contract = get_client_pallet(true, &config_for_test).await;
		eth_client_contract.register_submitter().await.unwrap();
		let mut beacon_rpc_client = BeaconRPCClient::new(
			&config_for_test.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		let eth1_rpc_client = Eth1RPCClient::new(&config_for_test.eth1_endpoint);
		let last_slot_searcher = LastSlotSearcher::new(true);

		let finalized_beacon_slot =
			eth_client_contract.get_finalized_beacon_block_slot().await.unwrap();

		send_execution_blocks(
			&beacon_rpc_client,
			&mut eth_client_contract,
			&eth1_rpc_client,
			finalized_beacon_slot + 1,
			config_for_test.slot_without_block - 2,
		)
		.await;

		let last_block_on_substrate = last_slot_searcher
			.binsearch_slot_forward(
				eth_client_contract.get_finalized_beacon_block_slot().await.unwrap() + 1,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block - 2);

		send_execution_blocks(
			&beacon_rpc_client,
			&mut eth_client_contract,
			&eth1_rpc_client,
			config_for_test.slot_without_block - 1,
			config_for_test.slot_without_block - 1,
		)
		.await;

		let last_block_on_substrate = last_slot_searcher
			.binsearch_slot_forward(
				eth_client_contract.get_finalized_beacon_block_slot().await.unwrap() + 1,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block);

		let last_block_on_substrate = last_slot_searcher
			.binsearch_slot_forward(
				eth_client_contract.get_finalized_beacon_block_slot().await.unwrap() + 1,
				config_for_test.slot_without_block,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block - 1);

		let last_block_on_substrate = last_slot_searcher
			.binsearch_slot_forward(
				config_for_test.slot_without_block,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block);

		beacon_rpc_client = BeaconRPCClient::new(
			"http://httpstat.us/504/",
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		if last_slot_searcher
			.binsearch_slot_forward(
				eth_client_contract.get_finalized_beacon_block_slot().await.unwrap() + 1,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.is_ok()
		{
			panic!("binarysearch returns result in unworking network");
		}
	}

	#[tokio::test]
	async fn test_binsearch_slot_search() {
		let config_for_test = get_test_config();
		let mut eth_client_contract = get_client_pallet(true, &config_for_test).await;
		eth_client_contract.register_submitter().await.unwrap();
		let mut beacon_rpc_client = BeaconRPCClient::new(
			&config_for_test.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		let eth1_rpc_client = Eth1RPCClient::new(&config_for_test.eth1_endpoint);
		let last_slot_searcher = LastSlotSearcher::new(true);

		let finalized_slot = eth_client_contract.get_finalized_beacon_block_slot().await.unwrap();

		send_execution_blocks(
			&beacon_rpc_client,
			&mut eth_client_contract,
			&eth1_rpc_client,
			finalized_slot + 1,
			config_for_test.slot_without_block - 2,
		)
		.await;

		let finalized_slot = eth_client_contract.get_finalized_beacon_block_slot().await.unwrap();

		let last_block_on_substrate = last_slot_searcher
			.binary_slot_search(
				finalized_slot + 1,
				finalized_slot,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block - 2);

		send_execution_blocks(
			&beacon_rpc_client,
			&mut eth_client_contract,
			&eth1_rpc_client,
			config_for_test.slot_without_block - 1,
			config_for_test.slot_without_block - 1,
		)
		.await;

		let last_block_on_substrate = last_slot_searcher
			.binary_slot_search(
				finalized_slot + 1,
				finalized_slot,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block);

		let last_block_on_substrate = last_slot_searcher
			.binary_slot_search(
				finalized_slot + 1,
				finalized_slot,
				config_for_test.slot_without_block,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block);

		let last_block_on_substrate = last_slot_searcher
			.binary_slot_search(
				finalized_slot + 1,
				finalized_slot,
				config_for_test.slot_without_block - 1,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block - 1);

		let last_block_on_substrate = last_slot_searcher
			.binary_slot_search(
				config_for_test.slot_without_block,
				finalized_slot,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block - 1);

		beacon_rpc_client = BeaconRPCClient::new(
			"http://httpstat.us/504/",
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		if last_slot_searcher
			.binary_slot_search(
				finalized_slot + 1,
				finalized_slot,
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.is_ok()
		{
			panic!("binarysearch returns result in unworking network");
		}
	}

	#[tokio::test]
	async fn test_get_last_slot_binsearch() {
		let config_for_test = get_test_config();
		let mut eth_client_contract = get_client_pallet(true, &config_for_test).await;
		eth_client_contract.register_submitter().await.unwrap();
		let mut beacon_rpc_client = BeaconRPCClient::new(
			&config_for_test.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		let eth1_rpc_client = Eth1RPCClient::new(&config_for_test.eth1_endpoint);
		let mut last_slot_searcher = LastSlotSearcher::new(true);

		let finalized_slot = eth_client_contract.get_finalized_beacon_block_slot().await.unwrap();
		send_execution_blocks(
			&beacon_rpc_client,
			&mut eth_client_contract,
			&eth1_rpc_client,
			finalized_slot + 1,
			config_for_test.slot_without_block - 2,
		)
		.await;

		let last_block_on_substrate = last_slot_searcher
			.get_last_slot(
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block - 2);

		send_execution_blocks(
			&beacon_rpc_client,
			&mut eth_client_contract,
			&eth1_rpc_client,
			config_for_test.slot_without_block - 1,
			config_for_test.slot_without_block - 1,
		)
		.await;

		let last_block_on_substrate = last_slot_searcher
			.get_last_slot(
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block);

		beacon_rpc_client = BeaconRPCClient::new(
			"http://httpstat.us/504/",
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		if last_slot_searcher
			.get_last_slot(
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.is_ok()
		{
			panic!("binarysearch returns result in unworking network");
		}
	}

	#[tokio::test]
	async fn test_get_last_slot_linearsearch() {
		let config_for_test = get_test_config();
		let mut eth_client_contract = get_client_pallet(true, &config_for_test).await;
		eth_client_contract.register_submitter().await.unwrap();
		let mut beacon_rpc_client = BeaconRPCClient::new(
			&config_for_test.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		let eth1_rpc_client = Eth1RPCClient::new(&config_for_test.eth1_endpoint);
		let mut last_slot_searcher = LastSlotSearcher::new(true);

		let finalized_slot = eth_client_contract.get_finalized_beacon_block_slot().await.unwrap();

		send_execution_blocks(
			&beacon_rpc_client,
			&mut eth_client_contract,
			&eth1_rpc_client,
			finalized_slot + 1,
			config_for_test.slot_without_block - 2,
		)
		.await;

		let last_block_on_substrate = last_slot_searcher
			.get_last_slot(
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block - 2);

		send_execution_blocks(
			&beacon_rpc_client,
			&mut eth_client_contract,
			&eth1_rpc_client,
			config_for_test.slot_without_block - 1,
			config_for_test.slot_without_block - 1,
		)
		.await;

		let last_block_on_substrate = last_slot_searcher
			.get_last_slot(
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.unwrap();
		assert_eq!(last_block_on_substrate, config_for_test.slot_without_block);

		beacon_rpc_client = BeaconRPCClient::new(
			"http://httpstat.us/504/",
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		if last_slot_searcher
			.get_last_slot(
				config_for_test.right_bound_in_slot_search,
				&beacon_rpc_client,
				&eth_client_contract,
			)
			.await
			.is_ok()
		{
			panic!("binarysearch returns result in unworking network");
		}
	}
}

fn to_error<T: std::fmt::Debug>(t: T) -> Box<dyn std::error::Error> {
	Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("{t:?}")))
}
