use super::*;
use crate as pallet_light_verifier;
use crate::mock::{Test, *};
use dkg_runtime_primitives::traits::OnSignedProposal;
use ethereum_types::Address;
use frame_support::{assert_err, assert_ok, bounded_vec, BoundedVec};
use pallet_bridge_registry::{types::BridgeMetadata, Bridges, ResourceToBridgeIndex};
use pallet_eth2_light_client::tests::{get_test_context, submit_and_check_execution_headers};
use sp_runtime::AccountId32;
use std::convert::TryFrom;
use webb_proposals::{self, evm, FunctionSignature, Nonce, ProposalHeader};

pub const GOERLI_CHAIN: TypedChainId = TypedChainId::Evm(5);
pub const ALICE: AccountId32 = AccountId32::new([1u8; 32]);

use crate::verify::TrieProver; // Import the TrieProver

#[test]
fn test_verify_storage_proof() {
	new_test_ext().execute_with(|| {
	// prep light client pallet
	let (headers, updates, _init_input) = get_test_context(None);
	assert_ok!(Eth2Client::submit_beacon_chain_light_client_update(
		RuntimeOrigin::signed(ALICE),
		GOERLI_CHAIN,
		updates[1].clone()
	));

	submit_and_check_execution_headers(
		pallet_eth2_light_client::mock::RuntimeOrigin::signed(ALICE),
		GOERLI_CHAIN,
		headers[0].iter().skip(1).rev().collect(),
	);

	let mut header = headers[0][1].clone();
	let block_hash = pallet_eth2_light_client::FinalizedExecutionBlocks::<Test>::get(
		GOERLI_CHAIN,
		header.number,
	);

	let hex = "56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421";
	header.hash = Some(eth_types::H256(array_bytes::hex_n_into::<H256, 32>(hex).unwrap()));

	let key : Vec<u8> = "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421".as_bytes().to_vec();
	let proof : Vec<Vec<u8>> = vec![
            "0xf90211a04d8422d764c3762a80fef76a2eff6c21ab12894e5e2d9e04f22c06ee318ac007a0301fdd1915952b8a7bea30127074e9e2f9ac9e014ee5d8fcf6152b272b1cd728a044ddd1b7a6507910a72ecb389812ccceb1f800e059f019688cc5755b27f1438ea05e3541c5d60667aa41a91ee48b9c507552c94cc0e9c569768efdc47a8d85550ba0f881971b55026fe4603b2d5a8b1301ce67530de59313bab3118667f24f1051fda00b91d2ed1088463f8657c7efd4fd14252eb276d690337c6fe7066ef71cb76fa1a07b6c47a2d3ec219f13aec4990bbf2a3eb55ea59813280919ae56799f7e77a380a08ef2469fbc1c656755ade5d4777e25c31b964c20b75e1ab08f7aed6f4e4cbd1da062d948f3cee681c1bdb25cf4a2287fb367f5e91a63b304451a576040b967137aa04c1de96d975aef98fc5cfa4f58c007297bebab155b684b8fb9ef1b6ae5d5d443a0b278ad72680d49f110f4fd687a0c09fda2157f42abeb055299ade27b2e63eb14a004555c709d71512dfe817aa70900fa10586db2033441dc663e4aa4bdfa0d3eaba0eb73902925859ab0e85b34ab26d78983163eeb71d0551094e3360414bda40820a0055b4e96de3c7460985024bcae7e7f985dd015bd844321a0d2ad1d719469c783a0815ebee417bb8309e04073bf1a9284fcc085f01c9c2ef2eddac6a48cdccbcf12a0f90a0e448523dc51a80acfc35b384927375141477d9bf8d0c7bc0b3d2b5ea02c80".as_bytes().to_vec(),
            "0xf90211a02ec53aa30b50dd55f9f8f9bf5428d596f779f2c185fd6477bfe5b4efadaae7d5a0a7a77593d8ff4aec38ed27625d5d6d14d1bf7a00ca10d562b8c8295cb5a9398ca001832c6104f429af06f56b77a90fcedb8da1f12349e8ee9a8af896947f891852a0892a46bf8f55c5f64b0af779b0aeed2b8d90755d6769726dfe13c57755d68d17a07c648d29d22d94302d05b5df82d2733fe6c5dea7eaa24623f7ec00bb95c3c18aa0eed7339a103cdde82d3bce5eb99d2a8098db1a37ea53052aa6e59112b2535d9ba066d4e48320199c41f5a9f8e680ad884f229d5a3f9e24b37d27760557f378498aa09635bc3bcb2b5ca9c434e66474f27b3d29c61132e1d48745a469c57e5b2e5442a0cf6394661bfcef70ad194cffbeddc3529a13bd447cc4a42239535cb4697eae4da0f1dd51a81c83d0e4d87ae6a2bcb4ea23a3781af95c80453e2ddb19a982bc4a6da0f33b16ced4ac6b7279ea3fc99f8f9e72b04d0348f01c983577f2f5d8d06cd313a0ef0e6a37a0605bf53bf4008f48dc6507bba802a5e26008172aad148245fea8caa0eec96c409876d9a82cd0046da97934c734ef0ef46aede447381cb3ccc9143b6ca0f85feed2292684af7f8370cd20911d583f3bd2c8b5f4b09587ff34240bafbfa7a0a461bdef60f617564d8ecad0a2c5d1f08e6e47832e4f46ad620206857c008586a0ccbea1835e26c34facad10a896aaba7dc1abb749b2ecb3870d80f324d8c88d5380".as_bytes().to_vec()
        ];

	// Call the function and check the result
	let result = <pallet_light_verifier::Pallet<Test> as ProofVerifier>::verify_storage_proof(header, key, proof);

	// Assert that the result is Ok(true)
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), true);
});
}
