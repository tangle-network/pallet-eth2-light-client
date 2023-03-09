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
use eth_types::{self, eth2::ExtendedBeaconBlockHeader};

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
	async fn get_last_submitted_slot(&self) -> Result<u64, eth2_pallet_init::Error> {
		let header = self.get_header().unwrap();
		Ok(header.header.slot)
	}

	async fn is_known_block(
		&self,
		execution_block_hash: &eth_types::H256,
	) -> Result<bool, eth2_pallet_init::Error> {
		Ok(Eth2Client::is_known_execution_header(self.network, *execution_block_hash))
	}

	async fn send_light_client_update(
		&mut self,
		light_client_update: eth_types::eth2::LightClientUpdate,
	) -> Result<(), eth2_pallet_init::Error> {
		Eth2Client::commit_light_client_update(self.network, light_client_update)
			.map_err(generic_error)?;
		Ok(())
	}

	async fn get_finalized_beacon_block_hash(
		&self,
	) -> Result<eth_types::H256, eth2_pallet_init::Error> {
		let header = self.get_header().map_err(generic_error)?;
		Ok(header.execution_block_hash)
	}

	async fn get_finalized_beacon_block_slot(&self) -> Result<u64, eth2_pallet_init::Error> {
		Ok(Eth2Client::finalized_beacon_block_slot(self.network))
	}

	async fn send_headers(
		&mut self,
		_headers: &[eth_types::BlockHeader],
		_end_slot: u64,
	) -> Result<(), eth2_pallet_init::Error> {
		todo!()
	}

	async fn get_min_deposit(&self) -> Result<Balance, eth2_pallet_init::Error> {
		Ok(0)
	}

	async fn register_submitter(&self) -> Result<(), eth2_pallet_init::Error> {
		let _ = Eth2Client::register_submitter(crate::mock::RuntimeOrigin::root(), self.network)
			.map_err(generic_error)?;
		Ok(())
	}

	async fn is_submitter_registered(
		&self,
		_account_id: Option<AccountId32>,
	) -> Result<bool, eth2_pallet_init::Error> {
		Ok(true)
	}

	async fn get_light_client_state(
		&self,
	) -> Result<eth_types::eth2::LightClientState, eth2_pallet_init::Error> {
		Ok(eth_types::eth2::LightClientState::default())
	}

	async fn get_num_of_submitted_blocks_by_account(&self) -> Result<u32, eth2_pallet_init::Error> {
		Ok(0)
	}

	async fn get_max_submitted_blocks_by_account(&self) -> Result<u32, eth2_pallet_init::Error> {
		Ok(100)
	}
}

fn generic_error<T: sp_std::fmt::Debug>(err: T) -> eth2_pallet_init::Error {
	eth2_pallet_init::Error::from(format!("{err:?}"))
}
