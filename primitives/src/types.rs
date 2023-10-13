#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use codec::{Decode, Encode};
use webb_proposals::ResourceId;
use scale_info::TypeInfo;

/// Represents a light proposal input.
///
/// This struct contains information needed for a light proposal, including the Ethereum block
/// header, the merkle root, various merkle proofs, leaf index, and the address of the vanchor.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct LightProposalInput {
	/// The Ethereum block header associated with the proposal.
	pub block_header: eth_types::BlockHeader,
	/// The merkle root of the proposal
	pub merkle_root: [u8; 32],
	/// The merkle proof for the root
	pub merkle_root_proof: Vec<Vec<u8>>,
	/// The index of the leaf in the merkle tree
	pub leaf_index: u32,
	/// The merkle proof for the leaf index
	pub leaf_index_proof: Vec<Vec<u8>>,
	/// The source resoure id
	pub resource_id: ResourceId,
}
