use super::*;

pub trait VerifyBlockHeaderExists {
	fn verify_block_header_exists(
		header: BlockHeader,
		typed_chain_id: TypedChainId,
	) -> Result<bool, DispatchError>;
}
