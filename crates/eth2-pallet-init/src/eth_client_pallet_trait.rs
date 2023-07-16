use async_trait::async_trait;
use eth_types::{
	eth2::{LightClientState, LightClientUpdate},
	pallet::ClientMode,
	primitives::FinalExecutionOutcomeView,
	BlockHeader, H256,
};
use subxt::utils::AccountId32;

pub type Balance = u128;

/// Interface for using Ethereum Light Client
#[async_trait]
pub trait EthClientPalletTrait<A = AccountId32>: Send + Sync + 'static {
	/// Submits the Light Client Update to Ethereum Light Client on Substrate. Returns the final
	/// execution outcome or an error
	fn send_light_client_update(
		&mut self,
		light_client_update: LightClientUpdate,
	) -> Result<FinalExecutionOutcomeView<crate::Error>, crate::Error>;

	/// Gets finalized beacon block hash from Ethereum Light Client on Substrate
	fn get_finalized_beacon_block_hash(&self) -> Result<H256, crate::Error>;

	/// Gets finalized beacon block slot from Ethereum Light Client on Substrate
	fn get_finalized_beacon_block_slot(&self) -> Result<u64, crate::Error>;

	/// Sends headers to Ethereum Light Client on Substrate. Returns final execution outcome or an error.
	///
	/// # Arguments
	///
	/// * `headers` - the list of headers for submission to Eth Client
	fn send_headers(
		&mut self,
		headers: &[BlockHeader],
	) -> Result<FinalExecutionOutcomeView<crate::Error>, Box<dyn std::error::Error>>;

	fn get_client_mode(&self) -> Result<ClientMode, crate::Error>;

	/// Gets the Light Client State of the Ethereum Light Client on Substrate
	fn get_light_client_state(&self) -> Result<LightClientState, crate::Error>;

	fn get_last_block_number(&self) -> Result<u64, crate::Error>;

	fn get_unfinalized_tail_block_number(&self) -> Result<Option<u64>, crate::Error>;
}
