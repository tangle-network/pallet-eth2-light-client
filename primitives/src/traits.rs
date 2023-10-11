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

/// A trait for verifying proofs related to storage and transactions.
pub trait ProofVerifier {
	/// Verify a storage proof against the provided block header, key, and proof.
	///
	/// # Parameters
	///
	/// - `header`: The block header against which the proof will be verified.
	/// - `key`: The key for which the proof is being verified.
	/// - `proof`: The proof data to be verified.
	///
	/// # Returns
	///
	/// Returns a `Result` where `Ok(true)` indicates that the proof is valid,
	/// while `Ok(false)` indicates that the proof is invalid. If an error occurs
	/// during the verification process, it will be returned as `Err`.
	fn verify_storage_proof(
		header: BlockHeader,
		key: Vec<u8>,
		proof: Vec<Vec<u8>>,
	) -> Result<bool, DispatchError>;

	/// Verify a transaction proof against the provided block header, key, and proof.
	///
	/// # Parameters
	///
	/// - `header`: The block header against which the proof will be verified.
	/// - `key`: The key for which the proof is being verified.
	/// - `proof`: The proof data to be verified.
	///
	/// # Returns
	///
	/// Returns a `Result` where `Ok(true)` indicates that the proof is valid,
	/// while `Ok(false)` indicates that the proof is invalid. If an error occurs
	/// during the verification process, it will be returned as `Err`.
	///
	/// # Note
	///
	/// This method is currently a placeholder and needs to be implemented.
	fn verify_transaction_proof(
		_header: BlockHeader,
		_key: Vec<u8>,
		_proof: Vec<Vec<u8>>,
	) -> Result<bool, DispatchError> {
		todo!()
	}
}
