// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use eth_types::BlockHeader;
use frame_support::pallet_prelude::DispatchError;
use webb_proposals::TypedChainId;
pub mod traits;
pub mod types;

// Fixed key for leaf index
pub const LEAF_INDEX_KEY: &[u8] =
	"0000001e0000000000000000c0ceba21bc4b08f134e1cbfdd75cb82a11f8b3fc".as_bytes();

// Fixed key for leaf index
pub const MERKLE_ROOT_KEY: &[u8] =
	"0000000000000000000000000000000000000000000000000000000000000000".as_bytes();
