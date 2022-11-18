use super::{
	eth2::{ExtendedBeaconBlockHeader, SyncCommittee},
	BlockHeader, H256,
};
use codec::{Decode, Encode};
use scale_info::TypeInfo;

/// Minimal information about a header.
#[derive(Clone, Encode, Decode, TypeInfo)]
pub struct ExecutionHeaderInfo<AccountId> {
	pub parent_hash: H256,
	pub block_number: u64,
	pub submitter: AccountId,
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, TypeInfo)]
pub struct InitInput<AccountId> {
	pub finalized_execution_header: BlockHeader,
	pub finalized_beacon_header: ExtendedBeaconBlockHeader,
	pub current_sync_committee: SyncCommittee,
	pub next_sync_committee: SyncCommittee,
	pub validate_updates: bool,
	pub verify_bls_signatures: bool,
	pub hashes_gc_threshold: u64,
	pub max_submitted_blocks_by_account: u32,
	pub trusted_signer: Option<AccountId>,
}
