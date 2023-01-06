use async_trait::async_trait;
use eth_types::{
	eth2::{LightClientState, LightClientUpdate},
	BlockHeader,
};
use sp_core::crypto::AccountId32;

pub type Balance = u128;

pub struct Eth2LightClientError {
	pub error: String
}

/// Interface for using Ethereum Light Client
#[async_trait]
pub trait EthClientPalletTrait {
	type Error;
	/// Returns the last submitted slot by this relay
	async fn get_last_submitted_slot(&self) -> u64;

	/// Checks if the block with the execution block hash is known to Ethereum Light Client on NEAR
	async fn is_known_block(
		&self,
		execution_block_hash: &eth_types::H256,
	) -> Result<bool, Self::Error>;

	/// Submits the Light Client Update to Ethereum Light Client on NEAR. Returns the final
	/// execution outcome or an error
	async fn send_light_client_update(
		&mut self,
		light_client_update: LightClientUpdate,
	) -> Result<(), Self::Error>;

	/// Gets finalized beacon block hash from Ethereum Light Client on NEAR
	async fn get_finalized_beacon_block_hash(&self) -> Result<eth_types::H256, Self::Error>;

	/// Gets finalized beacon block slot from Ethereum Light Client on NEAR
	async fn get_finalized_beacon_block_slot(&self) -> Result<u64, Self::Error>;

	/// Sends headers to Ethereum Light Client on NEAR. Returns final execution outcome or an error.
	///
	/// # Arguments
	///
	/// * `headers` - the list of headers for submission to Eth Client
	/// * `end_slot` - the slot of the last header in list
	async fn send_headers(
		&mut self,
		headers: &[BlockHeader],
		end_slot: u64,
	) -> Result<(), Self::Error>;

	/// Gets the minimum required deposit for the registration of a new relay
	async fn get_min_deposit(&self) -> Result<Balance, Self::Error>;

	/// Registers the current relay in the Ethereum Light Client on NEAR
	async fn register_submitter(&self) -> Result<(), Self::Error>;

	/// Checks if the relay is registered in the Ethereum Light Client on NEAR
	async fn is_submitter_registered(
		&self,
		account_id: Option<AccountId32>,
	) -> Result<bool, Self::Error>;

	/// Gets the Light Client State of the Ethereum Light Client on NEAR
	async fn get_light_client_state(&self) -> Result<LightClientState, Self::Error>;

	/// Get number of unfinalized blocks submitted by current relay and currently stored on contract
	async fn get_num_of_submitted_blocks_by_account(&self) -> Result<u32, Self::Error>;

	/// Get max possible number of unfinalized blocks which can be stored on contract for one
	/// account
	async fn get_max_submitted_blocks_by_account(&self) -> Result<u32, Self::Error>;
}
