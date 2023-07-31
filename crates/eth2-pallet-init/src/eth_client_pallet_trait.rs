use async_trait::async_trait;
use eth_types::{
	eth2::{LightClientState, LightClientUpdate},
	pallet::ClientMode,
	primitives::FinalExecutionOutcomeView,
	BlockHeader, H256,
};

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
	) -> anyhow::Result<FinalExecutionOutcomeView>;

	/// Gets finalized beacon block hash from Ethereum Light Client on Substrate
	async fn get_finalized_beacon_block_hash(&self) -> anyhow::Result<H256>;

	/// Gets finalized beacon block slot from Ethereum Light Client on Substrate
	async fn get_finalized_beacon_block_slot(&self) -> anyhow::Result<u64>;

	/// Sends headers to Ethereum Light Client on Substrate. Returns final execution outcome or an
	/// error.
	///
	/// # Arguments
	///
	/// * `headers` - the list of headers for submission to Eth Client
	async fn send_headers(
		&mut self,
		headers: &[BlockHeader],
	) -> anyhow::Result<FinalExecutionOutcomeView>;

	async fn get_client_mode(&self) -> anyhow::Result<ClientMode>;

	/// Gets the Light Client State of the Ethereum Light Client on Substrate
	async fn get_light_client_state(&self) -> anyhow::Result<LightClientState>;

	async fn get_last_block_number(&self) -> anyhow::Result<u64>;

	async fn get_unfinalized_tail_block_number(&self) -> anyhow::Result<Option<u64>>;
}
