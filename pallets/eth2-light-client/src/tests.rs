use crate::{mock::*, test_utils::*, Error, Lsb0, Paused};
use bitvec::bitarr;
use consensus::{EPOCHS_PER_SYNC_COMMITTEE_PERIOD, SLOTS_PER_EPOCH};
use eth_types::{eth2::LightClientUpdate, pallet::InitInput, BlockHeader, U256};
use frame_support::assert_ok;

use eth_types::H256;
use frame_support::assert_err;
use sp_runtime::AccountId32;

use webb_proposals::TypedChainId;

pub const MAINNET_CHAIN: TypedChainId = TypedChainId::Evm(1);
pub const GOERLI_CHAIN: TypedChainId = TypedChainId::Evm(5);
pub const ALICE: AccountId32 = AccountId32::new([1u8; 32]);

pub fn submit_and_check_execution_headers(
	origin: RuntimeOrigin,
	typed_chain_id: TypedChainId,
	headers: Vec<&BlockHeader>,
) {
	for header in headers {
		assert_ok!(Eth2Client::submit_execution_header(
			origin.clone(),
			typed_chain_id,
			header.clone()
		));
		assert!(Eth2Client::is_known_execution_header(typed_chain_id, header.number));
	}
}

pub fn get_test_context(
	init_options: Option<InitOptions<[u8; 32]>>,
) -> (&'static Vec<Vec<BlockHeader>>, &'static Vec<LightClientUpdate>, InitInput<[u8; 32]>) {
	let (headers, updates, init_input_0) = get_test_data(init_options);
	let init_input = init_input_0.clone().map_into();

	assert_ok!(Eth2Client::init(
		RuntimeOrigin::signed(ALICE.clone()),
		GOERLI_CHAIN,
		Box::new(init_input)
	));

	assert_eq!(Eth2Client::last_block_number(GOERLI_CHAIN), headers[0][0].number);

	(headers, updates, init_input_0)
}

mod generic_tests {
	use super::*;
	use hex::FromHex;
	use tree_hash::TreeHash;
	#[test]
	pub fn test_header_root() {
		let header =
			read_beacon_header(format!("./src/data/goerli/beacon_header_{}.json", 5258752));
		assert_eq!(
			H256(header.tree_hash_root()),
			Vec::from_hex("cd669c0007ab6ff261a02cc3335ba470088e92f0460bf1efac451009efb9ec0a")
				.unwrap()
				.into()
		);

		let header =
			read_beacon_header(format!("./src/data/mainnet/beacon_header_{}.json", 4100000));
		assert_eq!(
			H256(header.tree_hash_root()),
			Vec::from_hex("342ca1455e976f300cc96a209106bed2cbdf87243167fab61edc6e2250a0be6c")
				.unwrap()
				.into()
		);
	}

	#[test]
	pub fn test_submit_update_two_periods() {
		new_test_ext().execute_with(|| {
			let (headers, updates, _init_input) = get_test_context(None);
			frame_support::assert_ok!(Eth2Client::submit_beacon_chain_light_client_update(
				RuntimeOrigin::signed(ALICE),
				GOERLI_CHAIN,
				updates[1].clone()
			));

			submit_and_check_execution_headers(
				RuntimeOrigin::signed(ALICE),
				GOERLI_CHAIN,
				headers[0].iter().skip(1).rev().collect(),
			);

			for header in headers[0].iter().skip(1) {
				let header_hash = header.calculate_hash();
				assert!(
					Eth2Client::block_hash_safe(GOERLI_CHAIN, header.number).unwrap_or_default() ==
						header_hash,
					"Execution block hash is not finalized: {header_hash:?}"
				);
			}

			assert_eq!(
				Eth2Client::last_block_number(GOERLI_CHAIN),
				headers[0].last().unwrap().number
			);
		})
	}

	#[test]
	pub fn test_panic_on_submit_execution_block_from_fork_chain() {
		new_test_ext().execute_with(|| {
			let (headers, updates, _init_input) = get_test_context(None);
			assert_ok!(Eth2Client::submit_beacon_chain_light_client_update(
				RuntimeOrigin::signed(ALICE),
				GOERLI_CHAIN,
				updates[1].clone()
			));

			// Submit execution header with different hash
			let mut fork_header = headers[0][1].clone();
			// Difficulty is modified just in order to get a different header hash. Any other field
			// would be suitable too
			fork_header.difficulty = U256::from(ethereum_types::U256::from(99));
			assert_err!(
				Eth2Client::submit_execution_header(
					RuntimeOrigin::signed(ALICE),
					GOERLI_CHAIN,
					fork_header
				),
				Error::<Test>::BlockHashesDoNotMatch
			);
		});
	}

	#[test]
	pub fn test_gc_headers() {
		new_test_ext().execute_with(|| {
			let hashes_gc_threshold: usize = 9500;
			let (headers, updates, _init_input) = get_test_context(Some(InitOptions {
				validate_updates: true,
				verify_bls_signatures: true,
				hashes_gc_threshold: hashes_gc_threshold as u64,
				trusted_signer: None,
			}));
			assert_ok!(Eth2Client::submit_beacon_chain_light_client_update(
				RuntimeOrigin::signed(ALICE),
				GOERLI_CHAIN,
				updates[1].clone()
			));

			submit_and_check_execution_headers(
				RuntimeOrigin::signed(ALICE),
				GOERLI_CHAIN,
				headers[0].iter().skip(1).rev().collect(),
			);

			for header in headers[0].iter().skip(1) {
				assert!(
					Eth2Client::block_hash_safe(GOERLI_CHAIN, header.number).unwrap_or_default() ==
						header.calculate_hash(),
					"Execution block hash is not finalized: {:?}",
					header.calculate_hash()
				);
			}

			assert_ok!(Eth2Client::submit_beacon_chain_light_client_update(
				RuntimeOrigin::signed(ALICE),
				GOERLI_CHAIN,
				updates[2].clone()
			));

			submit_and_check_execution_headers(
				RuntimeOrigin::signed(ALICE),
				GOERLI_CHAIN,
				headers[1].iter().rev().collect(),
			);

			assert_eq!(
				Eth2Client::last_block_number(GOERLI_CHAIN),
				headers[1].last().unwrap().number
			);

			for header in headers[1].iter() {
				assert!(
					Eth2Client::block_hash_safe(GOERLI_CHAIN, header.number).unwrap_or_default() ==
						header.calculate_hash(),
					"Execution block hash is not finalized: {:?}",
					header.calculate_hash()
				);
			}

			for header in headers.concat().iter().rev().skip(hashes_gc_threshold + 2) {
				assert!(
					Eth2Client::block_hash_safe(GOERLI_CHAIN, header.number).is_none(),
					"Execution block hash was not removed: {:?}",
					header.calculate_hash()
				);
			}
		})
	}

	#[test]
	pub fn test_trusted_signer() {
		new_test_ext().execute_with(|| {
			let (_headers, updates, _init_input) = get_test_context(Some(InitOptions {
				validate_updates: true,
				verify_bls_signatures: true,
				hashes_gc_threshold: 7100,
				trusted_signer: Some([2u8; 32]),
			}));
			assert_err!(
				Eth2Client::submit_beacon_chain_light_client_update(
					RuntimeOrigin::signed(ALICE),
					GOERLI_CHAIN,
					updates[1].clone()
				),
				Error::<Test>::NotTrustedSigner,
			);
		});
	}

	#[test]
	pub fn test_panic_on_invalid_finality_proof() {
		new_test_ext().execute_with(|| {
			let (_headers, updates, _init_input) = get_test_context(None);
			let mut update = updates[1].clone();
			update.finality_update.finality_branch[5] = H256::from(
				hex::decode("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
					.unwrap(),
			);
			assert_err!(
				Eth2Client::submit_beacon_chain_light_client_update(
					RuntimeOrigin::signed(ALICE),
					GOERLI_CHAIN,
					update
				),
				Error::<Test>::InvalidFinalityProof,
			);
		});
	}

	#[test]
	pub fn test_panic_on_empty_finality_proof() {
		new_test_ext().execute_with(|| {
			let (_headers, updates, _init_input) = get_test_context(None);
			let mut update = updates[1].clone();
			update.finality_update.finality_branch = vec![];
			assert_err!(
				Eth2Client::submit_beacon_chain_light_client_update(
					RuntimeOrigin::signed(ALICE),
					GOERLI_CHAIN,
					update
				),
				Error::<Test>::InvalidFinalityProof,
			);
		});
	}

	#[test]
	pub fn test_panic_on_invalid_execution_block_proof() {
		new_test_ext().execute_with(|| {
			let (_headers, updates, _init_input) = get_test_context(None);
			let mut update = updates[1].clone();
			update.finality_update.header_update.execution_hash_branch[5] = H256::from(
				hex::decode("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
					.unwrap(),
			);
			assert_err!(
				Eth2Client::submit_beacon_chain_light_client_update(
					RuntimeOrigin::signed(ALICE),
					GOERLI_CHAIN,
					update
				),
				Error::<Test>::InvalidExecutionBlockHashProof
			);
		});
	}

	#[test]
	pub fn test_panic_on_empty_execution_block_proof() {
		new_test_ext().execute_with(|| {
			let (_headers, updates, _init_input) = get_test_context(None);
			let mut update = updates[1].clone();
			update.finality_update.header_update.execution_hash_branch = vec![];
			assert_err!(
				Eth2Client::submit_beacon_chain_light_client_update(
					RuntimeOrigin::signed(ALICE),
					GOERLI_CHAIN,
					update
				),
				Error::<Test>::InvalidExecutionBlockHashProof
			);
		});
	}

	#[test]
	pub fn test_panic_on_skip_update_period() {
		new_test_ext().execute_with(|| {
			let (_headers, updates, _init_input) = get_test_context(None);
			let mut update = updates[1].clone();
			update.finality_update.header_update.beacon_header.slot =
				update.signature_slot + EPOCHS_PER_SYNC_COMMITTEE_PERIOD * SLOTS_PER_EPOCH * 10;
			assert_err!(
				Eth2Client::submit_beacon_chain_light_client_update(
					RuntimeOrigin::signed(ALICE),
					GOERLI_CHAIN,
					update
				),
				Error::<Test>::UpdateHeaderSlotLessThanFinalizedHeaderSlot
			);
		});
	}

	#[test]
	pub fn test_panic_on_submit_update_with_missing_execution_blocks() {
		new_test_ext().execute_with(|| {
			let (headers, updates, _init_input) = get_test_context(None);
			assert_ok!(Eth2Client::submit_beacon_chain_light_client_update(
				RuntimeOrigin::signed(ALICE),
				GOERLI_CHAIN,
				updates[1].clone()
			));

			for (_index, header) in headers[0].iter().skip(1).take(5).enumerate() {
				assert_err!(
					Eth2Client::submit_execution_header(
						RuntimeOrigin::signed(ALICE),
						GOERLI_CHAIN,
						header.clone()
					),
					Error::<Test>::BlockHashesDoNotMatch
				);
			}
		});
	}

	#[test]
	pub fn test_panic_on_submit_same_execution_blocks() {
		new_test_ext().execute_with(|| {
			let (headers, updates, _init_input) = get_test_context(None);
			assert_ok!(Eth2Client::submit_beacon_chain_light_client_update(
				RuntimeOrigin::signed(ALICE),
				GOERLI_CHAIN,
				updates[1].clone()
			));
			assert_ok!(Eth2Client::submit_execution_header(
				RuntimeOrigin::signed(ALICE),
				GOERLI_CHAIN,
				headers[0].last().unwrap().clone()
			));
			assert_err!(
				Eth2Client::submit_execution_header(
					RuntimeOrigin::signed(ALICE),
					GOERLI_CHAIN,
					headers[0].last().unwrap().clone()
				),
				Error::<Test>::BlockHashesDoNotMatch
			);
		});
	}

	#[test]
	pub fn test_panic_on_submit_update_paused() {
		new_test_ext().execute_with(|| {
			let (_headers, updates, _init_input) = get_test_context(None);
			Paused::<Test>::insert(GOERLI_CHAIN, true);
			assert_err!(
				Eth2Client::submit_beacon_chain_light_client_update(
					RuntimeOrigin::signed(ALICE),
					GOERLI_CHAIN,
					updates[1].clone()
				),
				Error::<Test>::LightClientUpdateNotAllowed
			);
		});
	}

	#[test]
	pub fn test_panic_on_submit_outdated_update() {
		new_test_ext().execute_with(|| {
			let (_headers, updates, _init_input) = get_test_context(None);
			assert_err!(
				Eth2Client::submit_beacon_chain_light_client_update(
					RuntimeOrigin::signed(ALICE),
					GOERLI_CHAIN,
					updates[0].clone()
				),
				Error::<Test>::ActiveHeaderSlotLessThanFinalizedSlot,
			);
		});
	}

	#[test]
	pub fn test_panic_on_submit_blocks_with_unknown_parent() {
		new_test_ext().execute_with(|| {
			let (headers, updates, _init_input) = get_test_context(None);
			assert_eq!(Eth2Client::last_block_number(GOERLI_CHAIN), headers[0][0].number);
			assert_ok!(Eth2Client::submit_beacon_chain_light_client_update(
				RuntimeOrigin::signed(ALICE),
				GOERLI_CHAIN,
				updates[1].clone()
			));

			let tmp_headers: Vec<_> = headers[0].iter().skip(1).rev().collect();
			assert_ok!(Eth2Client::submit_execution_header(
				RuntimeOrigin::signed(ALICE),
				GOERLI_CHAIN,
				tmp_headers[0].clone()
			));
			// Skip 2th block
			assert_err!(
				Eth2Client::submit_execution_header(
					RuntimeOrigin::signed(ALICE),
					GOERLI_CHAIN,
					tmp_headers[3].clone()
				),
				Error::<Test>::BlockHashesDoNotMatch
			);
		});
	}

	#[test]
	// test_panic_on_submit_headers_in_worng_mode
	pub fn test_panic_on_submit_headers_in_wrong_mode() {
		new_test_ext().execute_with(|| {
			let (headers, _updates, _init_input) = get_test_context(None);
			assert_eq!(Eth2Client::last_block_number(GOERLI_CHAIN), headers[0][0].number);
			assert_err!(
				Eth2Client::submit_execution_header(
					RuntimeOrigin::signed(ALICE),
					GOERLI_CHAIN,
					headers[0][1].clone()
				),
				Error::<Test>::InvalidClientMode
			);
		});
	}

	#[test]
	pub fn test_panic_on_sync_committee_bits_is_less_than_threshold() {
		new_test_ext().execute_with(|| {
			let (_headers, updates, _init_input) = get_test_context(None);
			let mut update = updates[1].clone();

			let mut sync_committee_bits = bitarr![u8, Lsb0; 0; 512];

			// The number of participants should satisfy the inequality:
			// num_of_participants * 3 >= sync_committee_bits_size * 2
			// If the sync_committee_bits_size = 512, then
			// the minimum allowed value of num_of_participants is 342.

			// Fill the sync_committee_bits with 341 participants to trigger panic
			let num_of_participants = (((512.0 * 2.0 / 3.0) as f32).ceil() - 1.0) as usize;
			sync_committee_bits.get_mut(0..num_of_participants).unwrap().fill(true);
			update.sync_aggregate.sync_committee_bits =
				sync_committee_bits.as_raw_mut_slice().to_vec().into();
			assert_err!(
				Eth2Client::submit_beacon_chain_light_client_update(
					RuntimeOrigin::signed(ALICE),
					GOERLI_CHAIN,
					update
				),
				Error::<Test>::SyncCommitteeBitsSumLessThanThreshold,
			);
		});
	}

	#[test]
	pub fn test_panic_on_missing_sync_committee_update() {
		new_test_ext().execute_with(|| {
			let (_headers, updates, _init_input) = get_test_context(None);
			let mut update = updates[1].clone();
			update.sync_committee_update = None;

			assert_err!(
				Eth2Client::submit_beacon_chain_light_client_update(
					RuntimeOrigin::signed(ALICE),
					GOERLI_CHAIN,
					update
				),
				Error::<Test>::SyncCommitteeUpdateNotPresent
			);
		});
	}
}

mod mainnet_tests {
	use super::*;
	#[test]
	pub fn test_panic_on_init_in_trustless_mode_without_bls_on_mainnet() {
		new_test_ext().execute_with(|| {
			let (_headers, _updates, init_input) = get_test_data(Some(InitOptions {
				validate_updates: true,
				verify_bls_signatures: false,
				hashes_gc_threshold: 500,
				trusted_signer: None,
			}));

			assert_err!(
				Eth2Client::init(
					RuntimeOrigin::signed(ALICE),
					MAINNET_CHAIN,
					Box::new(init_input.map_into())
				),
				Error::<Test>::TrustlessModeError,
			);
		})
	}
}
