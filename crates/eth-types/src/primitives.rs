use codec::{Decode, Encode};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Encode, Decode)]
pub enum FinalExecutionStatus<E> {
	/// The execution has not yet started.
	NotStarted,
	/// The execution has started and still going.
	Started,
	/// The execution has failed with the given error.
	Failure(E),
	/// The execution has succeeded
	Success,
}

impl<E> Default for FinalExecutionStatus<E> {
	fn default() -> Self {
		FinalExecutionStatus::NotStarted
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Encode, Decode)]
pub struct FinalExecutionOutcomeView<E> {
	/// Execution status. Contains the result in case of successful execution.
	pub status: FinalExecutionStatus<E>,
}
