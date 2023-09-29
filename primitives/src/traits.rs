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
