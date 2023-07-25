use async_trait::async_trait;
use sp_runtime::AccountId32;

use crate::{
	mock::{Eth2Client, RuntimeOrigin},
	test_utils::{get_test_data, InitOptions},
	tests::ALICE,
};
use eth2_pallet_init::eth_client_pallet_trait::EthClientPalletTrait;
use eth_types::{
	self,
	eth2::{LightClientState, LightClientUpdate},
	pallet::ClientMode,
	primitives::{FinalExecutionOutcomeView, FinalExecutionStatus},
	BlockHeader, H256,
};
use frame_support::assert_ok;
use std::error::Error;
use webb_proposals::TypedChainId;

pub struct MockEthClientPallet {
	network: TypedChainId,
}

impl MockEthClientPallet {
	#[allow(dead_code)]
	pub fn init(&self, _typed_chain_id: TypedChainId, init_options: Option<InitOptions<[u8; 32]>>) {
		let (_headers, _updates, init_input) = get_test_data(init_options);
		assert_ok!(Eth2Client::init(
			RuntimeOrigin::signed(ALICE.clone()),
			self.network,
			Box::new(init_input.map_into())
		));
	}
}

#[async_trait]
impl EthClientPalletTrait<AccountId32> for MockEthClientPallet {
	async fn send_light_client_update(
		&mut self,
		_light_client_update: LightClientUpdate,
	) -> Result<FinalExecutionOutcomeView<Box<dyn Error>>, Box<dyn Error>> {
		Ok(FinalExecutionOutcomeView {
			status: FinalExecutionStatus::NotStarted,
			transaction_hash: Some(H256::from([0u8; 32])),
		})
	}

	async fn get_finalized_beacon_block_hash(&self) -> Result<H256, Box<dyn Error>> {
		Ok(H256::from([0u8; 32]))
	}

	async fn get_finalized_beacon_block_slot(&self) -> Result<u64, Box<dyn Error>> {
		Ok(0)
	}

	async fn send_headers(
		&mut self,
		_headers: &[BlockHeader],
	) -> Result<FinalExecutionOutcomeView<Box<dyn Error>>, Box<dyn Error>> {
		Ok(FinalExecutionOutcomeView {
			status: FinalExecutionStatus::NotStarted,
			transaction_hash: Some(H256::from([0u8; 32])),
		})
	}

	async fn get_client_mode(&self) -> Result<ClientMode, Box<dyn Error>> {
		Ok(ClientMode::default())
	}

	async fn get_light_client_state(&self) -> Result<LightClientState, Box<dyn Error>> {
		Ok(LightClientState::default())
	}

	async fn get_last_block_number(&self) -> Result<u64, Box<dyn Error>> {
		Ok(0)
	}

	async fn get_unfinalized_tail_block_number(&self) -> Result<Option<u64>, Box<dyn Error>> {
		Ok(None)
	}
}
