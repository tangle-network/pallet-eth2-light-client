use super::*;
use codec::{Decode, Encode};
use scale_info::TypeInfo;

/// Minimal information about a header.
#[derive(Clone, Encode, Decode, TypeInfo)]
pub struct ExecutionHeaderInfo<AccountId> {
    pub parent_hash: H256,
    pub block_number: u64,
    pub submitter: AccountId,
}
