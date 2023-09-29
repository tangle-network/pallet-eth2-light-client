// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use eth_types::BlockHeader;
use frame_support::{pallet_prelude::DispatchError, traits::Get, BoundedVec};
use webb_proposals::TypedChainId;
pub mod traits;
pub mod types;