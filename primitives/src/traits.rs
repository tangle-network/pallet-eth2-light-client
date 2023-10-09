use super::*;

/// A trait for exposing the light client functionality to external pallets
pub trait LightClientHandler {
	/// Verifies the existence of a block header on a specified typed chain.
	///
	/// # Parameters
	///
	/// * `header`: The block header to be verified.
	/// * `typed_chain_id`: The identifier of the typed chain on which the header is to be checked.
	///
	/// # Returns
	///
	/// Returns a `Result` containing a `bool` indicating whether the block header exists.
	///
	/// If the verification is successful, `Ok(true)` is returned. Otherwise, an error of type
	/// `DispatchError` is returned indicating the reason for the failure.
	fn verify_block_header_exists(
		header: BlockHeader,
		typed_chain_id: TypedChainId,
	) -> Result<bool, DispatchError>;
}

/// A trait for verifying storage proof for Ethereum storage.
pub trait StorageProofVerifier {
	/// Verifies the storage proof for Ethereum storage.
	///
	/// # Arguments
	///
	/// * `header` - The block header containing information about the block.
	/// * `typed_chain_id` - The typed chain ID indicating the type of Ethereum chain.
	/// * `root_merkle_proof` - The Merkle proof for the root of the storage trie.
	/// * `storage_merkle_proof` - The Merkle proof for the requested storage item.
	///
	/// # Returns
	///
	/// Returns `Ok(true)` if the storage proof is valid, indicating that the requested
	/// storage item is present in the Ethereum state. Returns `Ok(false)` if the storage
	/// proof is invalid or the item is not present. Returns an `Err` variant if there is
	/// an error during the verification process.
	fn verify_storage_proof(
		header: BlockHeader,
		typed_chain_id: TypedChainId,
		root_merkle_proof: Vec<u8>,
		storage_merkle_proof: Vec<u8>,
	) -> Result<bool, DispatchError>;
}
