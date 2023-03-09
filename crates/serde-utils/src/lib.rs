// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(feature = "std")]
mod quoted_int;

#[cfg(feature = "std")]
pub mod fixed_bytes_hex;
#[cfg(feature = "std")]
pub mod hex;
#[cfg(feature = "std")]
pub mod hex_vec;
#[cfg(feature = "std")]
pub mod json_str;
#[cfg(feature = "std")]
pub mod list_of_bytes_lists;
#[cfg(feature = "std")]
pub mod quoted_u64_vec;
#[cfg(feature = "std")]
pub mod u256_hex_be;
#[cfg(feature = "std")]
pub mod u32_hex;
#[cfg(feature = "std")]
pub mod u64_hex_be;
#[cfg(feature = "std")]
pub mod u8_hex;

#[cfg(feature = "std")]
pub use fixed_bytes_hex::{bytes_4_hex, bytes_8_hex};

#[cfg(feature = "std")]
pub use quoted_int::{quoted_u256, quoted_u32, quoted_u64, quoted_u8};
