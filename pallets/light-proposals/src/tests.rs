#![allow(clippy::unwrap_used)]
use crate::{mock::*, Error, LightProposalInput, Proposal, ProposalKind, ResourceId};
use dkg_runtime_primitives::{traits::OnSignedProposal, TypedChainId};
use ethereum_types::Address;
use frame_support::{assert_err, assert_ok, bounded_vec};
use lazy_static::lazy_static;
use pallet_bridge_registry::{types::BridgeMetadata, Bridges, ResourceToBridgeIndex};
use pallet_eth2_light_client::tests::{get_test_context, submit_and_check_execution_headers};
use sp_runtime::AccountId32;
use webb_proposals::{self, evm, FunctionSignature, Nonce, ProposalHeader, TargetSystem};

pub const GOERLI_CHAIN: TypedChainId = TypedChainId::Evm(5);
pub const ALICE: AccountId32 = AccountId32::new([1u8; 32]);

lazy_static! {
	static ref GOERLI_RESOURCE_ID: ResourceId =
		ResourceId::new(TargetSystem::new_contract_address(Address::from([0; 20])), GOERLI_CHAIN);
}

// setup bridge registry pallets with defaults
fn setup_bridge_registry() {
	let target_chain = webb_proposals::TypedChainId::Evm(1);
	let target_system = webb_proposals::TargetSystem::new_contract_address([1u8; 20]);
	let target_resource_id = webb_proposals::ResourceId::new(target_system, target_chain);
	// Create src info
	let src_chain = GOERLI_CHAIN;
	let src_target_system = webb_proposals::TargetSystem::new_contract_address([0u8; 20]);
	let src_resource_id = webb_proposals::ResourceId::new(src_target_system, src_chain);
	// Create mocked signed EVM anchor update proposals
	let proposal = evm::AnchorUpdateProposal::new(
		ProposalHeader::new(target_resource_id, FunctionSignature([0u8; 4]), Nonce(1)),
		[1u8; 32],
		src_resource_id,
	);
	let signed_proposal = Proposal::Signed {
		kind: ProposalKind::AnchorUpdate,
		data: proposal.into_bytes().to_vec().try_into().unwrap(),
		signature: vec![].try_into().unwrap(),
	};
	// Handle signed proposal
	assert_ok!(BridgeRegistry::on_signed_proposal(signed_proposal));
	// Verify the storage system updates correctly
	assert_eq!(ResourceToBridgeIndex::<Test>::get(target_resource_id), Some(1));
	assert_eq!(ResourceToBridgeIndex::<Test>::get(src_resource_id), Some(1));
	assert_eq!(
		Bridges::<Test>::get(1).unwrap(),
		BridgeMetadata {
			resource_ids: bounded_vec![src_resource_id, target_resource_id],
			info: Default::default()
		}
	);
}

#[test]
fn test_light_light_proposal_flow() {
	new_test_ext().execute_with(|| {
		// setup bridge registry with defaults
		setup_bridge_registry();

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
		header.hash = block_hash;

		// setup light client payload
		let light_proposal = LightProposalInput {
			block_header: header,
			merkle_root: [0; 32],
			merkle_root_proof: vec![vec![0; 32]],
			leaf_index: 0,
			leaf_index_proof: vec![vec![0; 32]],
			resource_id: *GOERLI_RESOURCE_ID,
		};

		assert_ok!(LightProposals::submit_proposal(RuntimeOrigin::signed(ALICE), light_proposal));
	});
}

#[test]
fn test_light_light_should_reject_if_header_is_not_present() {
	new_test_ext().execute_with(|| {
		// setup bridge registry with defaults
		setup_bridge_registry();

		let (headers, _updates, _init_input) = get_test_context(None);

		// setup light client payload
		let light_proposal = LightProposalInput {
			block_header: headers[0][1].clone(),
			merkle_root: [0; 32],
			merkle_root_proof: vec![vec![0; 32]],
			leaf_index: 0,
			leaf_index_proof: vec![vec![0; 32]],
			resource_id: *GOERLI_RESOURCE_ID,
		};

		assert_err!(
			LightProposals::submit_proposal(RuntimeOrigin::signed(ALICE), light_proposal),
			pallet_eth2_light_client::Error::<Test>::HeaderHashDoesNotExist
		);
	});
}

#[test]
fn test_light_light_should_reject_if_proof_verification_fails() {
	new_test_ext().execute_with(|| {
		// setup bridge registry with defaults
		setup_bridge_registry();

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
		header.hash = block_hash;

		// setup light client payload
		let light_proposal = LightProposalInput {
			block_header: header,
			merkle_root: [0; 32],
			merkle_root_proof: vec![vec![123]],
			leaf_index: 0,
			leaf_index_proof: vec![vec![0; 32]],
			resource_id: *GOERLI_RESOURCE_ID,
		};

		assert_err!(
			LightProposals::submit_proposal(RuntimeOrigin::signed(ALICE), light_proposal),
			Error::<Test>::ProofVerificationFailed
		);
	});
}
