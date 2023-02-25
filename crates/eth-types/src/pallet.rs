use crate::{
	eth2::{ExtendedBeaconBlockHeader, SyncCommittee},
	BlockHeader,
};
use codec::{Decode, Encode};
use ethereum_types::H256;
use scale_info::TypeInfo;

/// Minimal information about a header.
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode, TypeInfo)]
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

impl<AccountId> InitInput<AccountId> {
	pub fn map_into<R: From<AccountId>>(self) -> InitInput<R> {
		let trusted_signer = self.trusted_signer.map(R::from);
		InitInput {
			finalized_execution_header: self.finalized_execution_header,
			finalized_beacon_header: self.finalized_beacon_header,
			current_sync_committee: self.current_sync_committee,
			next_sync_committee: self.next_sync_committee,
			validate_updates: self.validate_updates,
			verify_bls_signatures: self.verify_bls_signatures,
			hashes_gc_threshold: self.hashes_gc_threshold,
			max_submitted_blocks_by_account: self.max_submitted_blocks_by_account,
			trusted_signer,
		}
	}
}
