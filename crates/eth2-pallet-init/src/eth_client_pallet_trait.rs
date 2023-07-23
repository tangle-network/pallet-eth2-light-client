use async_trait::async_trait;
use eth_types::{
	eth2::{LightClientState, LightClientUpdate},
	pallet::ClientMode,
	primitives::FinalExecutionOutcomeView,
	BlockHeader, H256,
};
use std::error::Error;
use webb::substrate::subxt::utils::AccountId32;

pub type Balance = u128;

/// Interface for using Ethereum Light Client
#[async_trait]
pub trait EthClientPalletTrait<A = AccountId32>: Send + Sync + 'static {
	/// Submits the Light Client Update to Ethereum Light Client on Substrate. Returns the final
	/// execution outcome or an error
	async fn send_light_client_update(
		&mut self,
		light_client_update: LightClientUpdate,
	) -> Result<FinalExecutionOutcomeView<Box<dyn Error>>, Box<dyn Error>>;

	/// Gets finalized beacon block hash from Ethereum Light Client on Substrate
	async fn get_finalized_beacon_block_hash(&self) -> Result<H256, Box<dyn Error>>;

	/// Gets finalized beacon block slot from Ethereum Light Client on Substrate
	async fn get_finalized_beacon_block_slot(&self) -> Result<u64, Box<dyn Error>>;

	/// Sends headers to Ethereum Light Client on Substrate. Returns final execution outcome or an
	/// error.
	///
	/// # Arguments
	///
	/// * `headers` - the list of headers for submission to Eth Client
	async fn send_headers(
		&mut self,
		headers: &[BlockHeader],
	) -> Result<FinalExecutionOutcomeView<Box<dyn Error>>, Box<dyn Error>>;

	async fn get_client_mode(&self) -> Result<ClientMode, Box<dyn Error>>;

	/// Gets the Light Client State of the Ethereum Light Client on Substrate
	async fn get_light_client_state(&self) -> Result<LightClientState, Box<dyn Error>>;

	async fn get_last_block_number(&self) -> Result<u64, Box<dyn Error>>;

	async fn get_unfinalized_tail_block_number(&self) -> Result<Option<u64>, Box<dyn Error>>;
}
