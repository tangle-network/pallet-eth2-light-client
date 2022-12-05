use async_trait::async_trait;

use crate::{
	eth_types::{
		self,
		eth2::{ExtendedBeaconBlockHeader, SyncCommittee},
		pallet::InitInput,
		BlockHeader,
	},
	mock::RuntimeOrigin,
	test_utils::{get_test_data, InitOptions},
	tests::{ALICE, KILN_CHAIN},
};
use eth2_pallet_init::eth_network::EthNetwork;
use frame_support::assert_ok;
use sp_core::{crypto::AccountId32, sr25519::Pair, Decode};
use webb_proposals::TypedChainId;

use crate::mock::Eth2Client;
use eth2_pallet_init::eth_client_pallet_trait::EthClientPalletTrait;

pub struct EthClientPallet {
	network: TypedChainId,
}

impl EthClientPallet {
	pub fn init(
		&self,
		_typed_chain_id: TypedChainId,
		init_options: Option<InitOptions<AccountId32>>,
	) {
		let (headers, updates, init_input) = get_test_data(init_options);
		assert_ok!(Eth2Client::init(
			RuntimeOrigin::signed(ALICE.clone()),
			self.network,
			Box::new(init_input.clone())
		));
	}
}

#[async_trait]
impl EthClientPalletTrait for EthClientPallet {
	async fn get_last_submitted_slot(&self) -> u64 {
		let header: ExtendedBeaconBlockHeader =
			Eth2Client::finalized_beacon_header(self.network).unwrap();
		let slot = header.header.slot;
		slot
	}

	async fn is_known_block(
		&self,
		_execution_block_hash: &eth_types::H256,
	) -> Result<bool, Box<dyn std::error::Error>> {
		Ok(false)
	}

	async fn send_light_client_update(
		&mut self,
		_light_client_update: eth_types::eth2::LightClientUpdate,
	) -> Result<(), Box<dyn std::error::Error>> {
		Ok(())
	}

	async fn get_finalized_beacon_block_hash(
		&self,
	) -> Result<eth_types::H256, Box<dyn std::error::Error>> {
		Ok(eth_types::H256::from([0u8; 32]))
	}

	async fn get_finalized_beacon_block_slot(&self) -> Result<u64, Box<dyn std::error::Error>> {
		Ok(0)
	}

	async fn send_headers(
		&mut self,
		_headers: &[eth_types::BlockHeader],
		_end_slot: u64,
	) -> Result<(), Box<dyn std::error::Error>> {
		Ok(())
	}

	async fn get_min_deposit(
		&self,
	) -> Result<crate::eth_client_pallet_trait::Balance, Box<dyn std::error::Error>> {
		Ok(0)
	}

	async fn register_submitter(&self) -> Result<(), Box<dyn std::error::Error>> {
		Ok(())
	}

	async fn is_submitter_registered(
		&self,
		_account_id: Option<AccountId32>,
	) -> Result<bool, Box<dyn std::error::Error>> {
		Ok(true)
	}

	async fn get_light_client_state(
		&self,
	) -> Result<eth_types::eth2::LightClientState, Box<dyn std::error::Error>> {
		Ok(eth_types::eth2::LightClientState::default())
	}

	async fn get_num_of_submitted_blocks_by_account(
		&self,
	) -> Result<u32, Box<dyn std::error::Error>> {
		Ok(0)
	}

	async fn get_max_submitted_blocks_by_account(&self) -> Result<u32, Box<dyn std::error::Error>> {
		Ok(0)
	}
}
