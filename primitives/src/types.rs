use super::*;

#[cfg(feature = "std")]
use serde::{Serialize, Deserialize};

use codec::{Encode, Decode, MaxEncodedLen};
use scale_info::TypeInfo;
use frame_support::RuntimeDebug;

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct LightProposalInput<MaxProofSize : Get<u32>> {
    pub block_header : eth_types::BlockHeader,
    pub root_merkle_proof : BoundedVec<u8, MaxProofSize>,
    pub storage_merkle_proof : BoundedVec<u8, MaxProofSize>,
} 