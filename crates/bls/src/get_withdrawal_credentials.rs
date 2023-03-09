use crate::PublicKey;
use alloc::{vec, vec::Vec};
use eth2_hashing::hash;
use eth2_ssz::Encode;

/// Returns the withdrawal credentials for a given public key.
///
/// Used for submitting deposits to the Eth1 deposit contract.
pub fn get_withdrawal_credentials(pubkey: &PublicKey, prefix_byte: u8) -> Vec<u8> {
	let hashed = hash(&pubkey.as_ssz_bytes());
	let mut prefixed = vec![prefix_byte];
	prefixed.extend_from_slice(&hashed[1..]);

	prefixed
}
