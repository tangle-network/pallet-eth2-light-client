use async_trait::async_trait;
use sp_runtime::AccountId32;

use crate::{
	mock::RuntimeOrigin,
	test_utils::{get_test_data, InitOptions},
	tests::ALICE,
};
use frame_support::assert_ok;
use webb_proposals::TypedChainId;

use crate::mock::Eth2Client;
use eth2_pallet_init::eth_client_pallet_trait::{Balance, EthClientPalletTrait};
use eth_types::{self, eth2::{ExtendedBeaconBlockHeader, LightClientState}, primitives::{FinalExecutionOutcomeView, FinalExecutionStatus}, H256, pallet::ClientMode};

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

	fn get_header(&self) -> Result<ExtendedBeaconBlockHeader, Box<dyn std::error::Error>> {
		Eth2Client::finalized_beacon_header(self.network).ok_or_else(|| {
			Box::new(std::io::Error::new(
				std::io::ErrorKind::Other,
				"Unable to obtain finalized beacon header",
			)) as Box<dyn std::error::Error>
		})
	}
}

#[async_trait]
impl EthClientPalletTrait<AccountId32> for MockEthClientPallet {
	fn send_light_client_update(
		&mut self,
		light_client_update: LightClientUpdate,
	) -> Result<FinalExecutionOutcomeView<crate::Error>, crate::Error> {
		Ok(FinalExecutionOutcomeView { status: FinalExecutionStatus::NotStarted })
	}

	fn get_finalized_beacon_block_hash(&self) -> Result<H256, crate::Error> {
		Ok(H256::zero())
	}

	fn get_finalized_beacon_block_slot(&self) -> Result<u64, crate::Error> {
		Ok(0)
	}

	fn send_headers(
		&mut self,
		headers: &[BlockHeader],
	) -> Result<FinalExecutionOutcomeView<crate::Error>, Box<dyn std::error::Error>> {
		Ok(FinalExecutionOutcomeView { status: FinalExecutionStatus::NotStarted })
	}

	fn get_client_mode(&self) -> Result<ClientMode, crate::Error> {
		Ok(ClientMode::default())
	}

	fn get_light_client_state(&self) -> Result<LightClientState, crate::Error> {
		Ok(LightClientState::default())
	}

	fn get_last_block_number(&self) -> Result<u64, crate::Error> {
		Ok(0)
	}

	fn get_unfinalized_tail_block_number(&self) -> Result<Option<u64>, crate::Error> {
		Ok(None)
	}
}

fn generic_error<T: sp_std::fmt::Debug>(err: T) -> eth2_pallet_init::Error {
	eth2_pallet_init::Error::from(format!("{err:?}"))
}
