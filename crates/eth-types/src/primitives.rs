use codec::{Decode, Encode};

use crate::H256;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Encode, Decode, Default)]
pub enum FinalExecutionStatus {
	/// The execution has not yet started.
	NotStarted,
	/// The execution has started and still going.
	Started,
	/// The execution has failed with the given error.
	Failure,
	/// The execution has succeeded
	#[default]
	Success,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct FinalExecutionOutcomeView {
	/// Execution status. Contains the result in case of successful execution.
	pub status: FinalExecutionStatus,
	/// Transaction hash,
	pub transaction_hash: Option<H256>,
}
