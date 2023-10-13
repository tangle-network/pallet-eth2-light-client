use super::*;
use dkg_runtime_primitives::H256;
use frame_support::ensure;
use rlp::Rlp;

/// A utility for working with trie proofs.
pub struct TrieProver;

impl TrieProver {
	/// Extract nibbles from a vector of bytes.
	///
	/// Each byte is split into two nibbles, resulting in a new vector.
	fn extract_nibbles(a: Vec<u8>) -> Vec<u8> {
		a.iter().flat_map(|b| vec![b >> 4, b & 0x0F]).collect()
	}

	/// Get an element at a specified position from RLP encoded data and decode it as a vector of
	/// bytes.
	fn get_vec(data: &Rlp, pos: usize) -> Result<Vec<u8>, String> {
		let data = data.at(pos).map_err(|_| "No data found!")?;
		data.as_val::<Vec<u8>>().map_err(|_| "Unable to decode data!".to_string())
	}

	/// Calculate the Keccak-256 hash of the given data.
	pub fn keccak_256(data: &[u8]) -> [u8; 32] {
		let mut buffer = [0u8; 32];
		let mut hasher = tiny_keccak::Keccak::v256();
		tiny_keccak::Hasher::update(&mut hasher, data);
		tiny_keccak::Hasher::finalize(hasher, &mut buffer);
		buffer
	}

	/// Verify a trie proof.
	///
	/// This function takes the expected root, a key, and a proof, and returns the value associated
	/// with the key in the trie.
	pub fn verify_trie_proof(
		expected_root: H256,
		key: Vec<u8>,
		proof: Vec<Vec<u8>>,
	) -> Result<Vec<u8>, String> {
		let mut actual_key = vec![];
		for el in key {
			actual_key.push(el / 16);
			actual_key.push(el % 16);
		}

		Self::_verify_trie_proof((expected_root.0).to_vec(), &actual_key, &proof, 0, 0)
	}

	fn _verify_trie_proof(
		expected_root: Vec<u8>,
		key: &Vec<u8>,
		proof: &Vec<Vec<u8>>,
		key_index: usize,
		proof_index: usize,
	) -> Result<Vec<u8>, String> {
		let raw_node = proof[proof_index].as_slice();

		if key_index == 0 {
			// trie root is always a hash
			ensure!(Self::keccak_256(raw_node) == expected_root.as_slice(), "Trie root mismatch!");
		} else if raw_node.len() < 32 {
			// if rlp < 32 bytes, then it is not hashed
			ensure!(raw_node == expected_root, "Trie root mismatch!");
		} else {
			ensure!(Self::keccak_256(raw_node) == expected_root.as_slice(), "Trie root mismatch!");
		}

		let node = Rlp::new(raw_node);

		if node.iter().count() == 17 {
			// Branch node
			if key_index == key.len() {
				ensure!(proof_index + 1 == proof.len(), "Branch index mismatch");
				Self::get_vec(&node, 16)
			} else {
				let new_expected_root = Self::get_vec(&node, key[key_index] as usize)?;
				//println!("new expected root {:?}", hex::encode(new_expected_root.as_slice()));
				Self::_verify_trie_proof(
					new_expected_root,
					key,
					proof,
					key_index + 1,
					proof_index + 1,
				)
			}
		} else {
			// Leaf or extension node
			ensure!(node.iter().count() == 2, "leaf count unexpected!");
			let path_u8 = Self::get_vec(&node, 0)?;

			// Extract first nibble
			let head = path_u8[0] / 16;
			ensure!(head <= 3, "leaf count unexpected!");

			// Extract path
			let mut path = vec![];
			if head % 2 == 1 {
				path.push(path_u8[0] % 16);
			}
			for val in path_u8.into_iter().skip(1) {
				path.push(val / 16);
				path.push(val % 16);
			}
			assert_eq!(path.as_slice(), &key[key_index..key_index + path.len()]);

			if head >= 2 {
				// Leaf node
				ensure!(proof_index + 1 == proof.len(), "Branch index mismatch");
				ensure!(key_index + path.len() == key.len(), "Branch index mismatch");
				Self::get_vec(&node, 1)
			} else {
				// Extension node
				let new_expected_root = Self::get_vec(&node, 1)?;
				Self::_verify_trie_proof(
					new_expected_root,
					key,
					proof,
					key_index + path.len(),
					proof_index + 1,
				)
			}
		}
	}
}
