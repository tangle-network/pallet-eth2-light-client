use super::*;
use eth_types::{LogEntry, Receipt};
use rlp::{Decodable, Encodable, Rlp};
pub trait Eth2Prover {
    fn verify_trie_proof(expected_root: H256, key: Vec<u8>, proof: Vec<Vec<u8>>) -> Vec<u8> {
        let mut actual_key = vec![];
        for el in key {
            actual_key.push(el / 16);
            actual_key.push(el % 16);
        }

        Self::_verify_trie_proof((expected_root.0 .0).into(), &actual_key, &proof, 0, 0)
    }

    fn verify_log_entry(
        log_index: u64,
        log_entry_data: Vec<u8>,
        receipt_index: u64,
        receipt_data: Vec<u8>,
        header_data: Vec<u8>,
        proof: Vec<Vec<u8>>,
    ) -> bool {
        let log_entry: LogEntry = rlp::decode(log_entry_data.as_slice()).unwrap();
        let receipt: Receipt = rlp::decode(receipt_data.as_slice()).unwrap();
        let header: BlockHeader = rlp::decode(header_data.as_slice()).unwrap();

        #[cfg(feature = "std")]
        {
            println!("log {:?}", log_entry);
            println!("receipt {:?}", receipt);
            println!("Header {:?}", header);
        }

        // Verify receipt included into header
        let verification_result = Self::verify_trie_proof(
            header.receipts_root,
            rlp::encode(&receipt_index).to_vec(),
            proof,
        );

        return verification_result == receipt_data;
    }

    fn _verify_trie_proof(
        expected_root: Vec<u8>,
        key: &Vec<u8>,
        proof: &Vec<Vec<u8>>,
        key_index: usize,
        proof_index: usize,
    ) -> Vec<u8>;
}
