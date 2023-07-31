// This file is part of Webb.
// Implementations are Substrate adaptations of the Rainbow Bridge.
// https://github.com/aurora-is-near/rainbow-bridge

// Copyright (C) 2022 Webb Technologies Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! # Eth2 Light Client Module
//! This pallet provides functionality for managing light client data on the Substrate chain.
//! It allows submitting beacon chain light client updates, submitting execution headers,
//! updating the trusted signer, and interacting with finalized and unfinalized headers.
//!
//! ## Overview
//!
//! The main functionality of this pallet is to manage light client data, including:
//! - Submitting beacon chain light client updates
//! - Submitting execution headers
//! - Updating the trusted signer
//! - Interacting with finalized and unfinalized headers
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! * `submit_beacon_chain_light_client_update` - Submit a beacon chain light client update.
//! * `submit_execution_header` - Submit an execution header.
//! * `update_trusted_signer` - Update the trusted signer account.
//!
//! ### Public Functions
//!
//! * `initialized` - Check if the light client is initialized for a specific chain.
//! * `last_block_number` - Get the last finalized execution block number for a given chain.
//! * `block_hash_safe` - Get the block hash for a given chain and block number, if available.
//! * `is_known_execution_header` - Check if an execution header is already submitted for a given
//!   chain and hash.
//! * `finalized_beacon_block_root` - Get the finalized beacon block root for a given chain.
//! * `finalized_beacon_block_slot` - Get the finalized beacon block slot for a given chain.
//! * `finalized_beacon_block_header` - Get the finalized beacon block header for a given chain.
//! * `get_light_client_state` - Get the current light client state for a given chain.
//! * `get_client_mode` - Get the current client mode for a given chain.
//! * `get_unfinalized_tail_block_number` - Get the unfinalized tail block number for a given chain.
//! * `get_trusted_signer` - Get the trusted signer account.
//! * `is_light_client_update_allowed` - Check if a light client update is allowed for a given
//!   submitter and chain.
//! * `validate_light_client_update` - Validate a light client update for a given chain.
//! * `verify_bls_signatures` - Verify the BLS signatures for a given chain, update, and sync
//!   committee bits.
//! * `verify_finality_branch` - Verify the finality branch of a light client update.
//! * `update_finalized_header` - Update the finalized header for a given chain.
//! * `commit_light_client_update` - Commit a light client update for a given chain.
//! * `account_id` - Get the account ID.
//!
//! ### Types
//!
//! * `Call` - Enum representing the dispatchable functions in this pallet.
//! * `Config` - A trait representing the configurable parameters of this pallet.
//! * `Error` - Enum representing the possible errors that can be returned by this pallet's
//!   functions.
//! * `Event` - Enum representing the possible events that can be emitted by this pallet.
//! * `LightClientUpdate` - Struct representing a light client update.
//! * `TypedChainId` - Enum representing the supported chains.
//! * `BlockHeader` - Struct representing a block header.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]
#![feature(slice_pattern)]

use eth_types::{
	eth2::{ExtendedBeaconBlockHeader, LightClientState, LightClientUpdate, SyncCommittee},
	pallet::{ClientMode, ExecutionHeaderInfo, InitInput},
	BlockHeader, H256,
};
use frame_support::{
	pallet_prelude::{ensure, DispatchError},
	traits::Get,
	PalletId,
};
use sp_std::{convert::TryInto, prelude::*};
use tree_hash::TreeHash;
use webb_proposals::TypedChainId;

pub use pallet::*;

use bitvec::prelude::{BitVec, Lsb0};

use frame_support::{sp_runtime::traits::AccountIdConversion, traits::Currency};

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod mocked_pallet_client;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod test_utils;

// pub mod consensus;
use consensus::{
	compute_domain, compute_signing_root, compute_sync_committee_period, convert_branch,
	get_participant_pubkeys, validate_beacon_block_header_update, DOMAIN_SYNC_COMMITTEE,
	FINALITY_TREE_DEPTH, FINALITY_TREE_INDEX, MIN_SYNC_COMMITTEE_PARTICIPANTS,
	SYNC_COMMITTEE_TREE_DEPTH, SYNC_COMMITTEE_TREE_INDEX,
};

pub mod traits;

pub use traits::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use consensus::network_config::NetworkConfig;
	use eth_types::{eth2::BeaconBlockHeader, pallet::ClientMode};
	use frame_support::{
		dispatch::DispatchResultWithPostInfo,
		pallet_prelude::{OptionQuery, ValueQuery, *},
		Blake2_128Concat,
	};
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	/// The module configuration trait.
	pub trait Config: frame_system::Config + pallet_balances::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		#[pallet::constant]
		type PalletId: Get<PalletId>;

		type Currency: Currency<Self::AccountId>;

		type StoragePricePerByte: Get<BalanceOf<Self>>;
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub networks: Vec<(TypedChainId, NetworkConfig)>,
		pub phantom: PhantomData<T>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { networks: vec![], phantom: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			for n in self.networks.clone() {
				match n.0 {
					TypedChainId::Evm(_) => {},
					_ => {
						panic!("Unsupported network type, ETH2 chains are only supported on EVM networks");
					},
				}
				<NetworkConfigForChain<T>>::insert(n.0, n.1);
			}
		}
	}

	/************* STORAGE ************ */

	/// If set, only light client updates by the trusted signer will be accepted
	#[pallet::storage]
	#[pallet::getter(fn trusted_signer)]
	pub(super) type TrustedSigner<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

	/// Mask determining all paused functions
	#[pallet::storage]
	#[pallet::getter(fn paused)]
	pub(super) type Paused<T: Config> =
		StorageMap<_, Blake2_128Concat, TypedChainId, bool, ValueQuery>;

	/// Whether the client validates the updates.
	/// Should only be set to `false` for debugging, testing, and diagnostic purposes
	#[pallet::storage]
	#[pallet::getter(fn validate_updates)]
	pub(super) type ValidateUpdates<T: Config> =
		StorageMap<_, Blake2_128Concat, TypedChainId, bool, ValueQuery>;

	/// Whether the client verifies BLS signatures.
	#[pallet::storage]
	#[pallet::getter(fn verify_bls_sigs)]
	pub(super) type VerifyBlsSignatures<T: Config> =
		StorageMap<_, Blake2_128Concat, TypedChainId, bool, ValueQuery>;

	/// We store the hashes of the blocks for the past `hashes_gc_threshold` headers.
	/// Events that happen past this threshold cannot be verified by the client.
	/// It is desirable that this number is larger than 7 days' worth of headers, which is roughly
	/// 51k Ethereum blocks. So this number should be 51k in production.
	#[pallet::storage]
	#[pallet::getter(fn hashes_gc_threshold)]
	pub(super) type HashesGcThreshold<T: Config> =
		StorageMap<_, Blake2_128Concat, TypedChainId, u64, ValueQuery>;

	/// Hashes of the finalized execution blocks mapped to their numbers. Stores up to
	/// `hashes_gc_threshold` entries. Execution block number -> execution block hash
	#[pallet::storage]
	#[pallet::getter(fn finalized_execution_blocks)]
	pub(super) type FinalizedExecutionBlocks<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		TypedChainId,
		Blake2_128Concat,
		u64,
		H256,
		OptionQuery,
	>;

	/// Light client state
	#[pallet::storage]
	#[pallet::getter(fn finalized_beacon_header)]
	pub(super) type FinalizedBeaconHeader<T: Config> =
		StorageMap<_, Blake2_128Concat, TypedChainId, ExtendedBeaconBlockHeader, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn finalized_execution_header)]
	pub(super) type FinalizedExecutionHeader<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		TypedChainId,
		ExecutionHeaderInfo<T::AccountId>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn current_sync_committee)]
	pub(super) type CurrentSyncCommittee<T: Config> =
		StorageMap<_, Blake2_128Concat, TypedChainId, SyncCommittee, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn next_sync_committee)]
	pub(super) type NextSyncCommittee<T: Config> =
		StorageMap<_, Blake2_128Concat, TypedChainId, SyncCommittee, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn unfinalized_head_execution_header)]
	pub(super) type UnfinalizedHeadExecutionHeader<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		TypedChainId,
		ExecutionHeaderInfo<T::AccountId>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn unfinalized_tail_execution_header)]
	pub(super) type UnfinalizedTailExecutionHeader<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		TypedChainId,
		ExecutionHeaderInfo<T::AccountId>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn client_mode)]
	pub(super) type ClientModeForChain<T: Config> =
		StorageMap<_, Blake2_128Concat, TypedChainId, ClientMode, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn network_config_for_chain)]
	pub(super) type NetworkConfigForChain<T: Config> =
		StorageMap<_, Blake2_128Concat, TypedChainId, NetworkConfig, OptionQuery>;

	/************* STORAGE ************ */

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Init {
			typed_chain_id: TypedChainId,
			header_info: ExecutionHeaderInfo<T::AccountId>,
		},
		SubmitBeaconChainLightClientUpdate {
			typed_chain_id: TypedChainId,
			submitter: T::AccountId,
			beacon_block_header: BeaconBlockHeader,
		},
		SubmitExecutionHeader {
			typed_chain_id: TypedChainId,
			header_info: Box<BlockHeader>,
		},
		UpdateTrustedSigner {
			trusted_signer: T::AccountId,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The light client is already initialized for the typed chain ID
		AlreadyInitialized,
		/// For attempting to update the light client
		LightClientUpdateNotAllowed,
		/// Block already submitted
		BlockAlreadySubmitted,
		/// Unknown parent block header hash
		UnknownParentHeader,
		/// Self-explanatory
		NotTrustedSigner,
		/// The updates validation can't be disabled for mainnet
		ValidateUpdatesParameterError,
		/// The client can't be executed in the trustless mode without BLS sigs verification on
		/// Mainnet
		TrustlessModeError,
		InvalidSyncCommitteeBitsSum,
		SyncCommitteeBitsSumLessThanThreshold,
		InvalidNetworkConfig,
		/// Failed to verify the bls signature
		InvalidBlsSignature,
		InvalidExecutionBlock,
		/// The active header slot number should be higher than the finalized slot
		ActiveHeaderSlotLessThanFinalizedSlot,
		/// The attested header slot should be equal to or higher than the finalized header slot
		UpdateHeaderSlotLessThanFinalizedHeaderSlot,
		/// The signature slot should be higher than the attested header slot
		UpdateSignatureSlotLessThanAttestedHeaderSlot,
		/// The acceptable update periods are not met.
		InvalidUpdatePeriod,
		/// Invalid finality proof
		InvalidFinalityProof,
		/// Invalid execution block hash proof
		InvalidExecutionBlockHashProof,
		NextSyncCommitteeNotPresent,
		InvalidNextSyncCommitteeProof,
		FinalizedExecutionHeaderNotPresent,
		FinalizedBeaconHeaderNotPresent,
		UnfinalizedHeaderNotPresent,
		SyncCommitteeUpdateNotPresent,
		HeaderHashDoesNotExist,
		/// The block hash does not match the expected block hash
		BlockHashesDoNotMatch,
		InvalidSignaturePeriod,
		CurrentSyncCommitteeNotSet,
		NextSyncCommitteeNotSet,
		/// The current client mode is invalid for the action.
		InvalidClientMode,
		/// "The `hashes_gc_threshold` is not enough to be able to apply gc correctly"
		HashesGcThresholdInsufficient,
		/// The chain cannot be closed
		ChainCannotBeClosed,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight({0})]
		#[pallet::call_index(0)]
		pub fn init(
			origin: OriginFor<T>,
			typed_chain_id: TypedChainId,
			args: Box<InitInput<T::AccountId>>,
		) -> DispatchResultWithPostInfo {
			let signer = ensure_signed(origin)?;
			ensure!(
				!<FinalizedBeaconHeader<T>>::contains_key(typed_chain_id),
				Error::<T>::AlreadyInitialized
			);

			if typed_chain_id == TypedChainId::Evm(1) {
				ensure!(
					args.validate_updates,
					// The updates validation can't be disabled for mainnet
					Error::<T>::ValidateUpdatesParameterError,
				);
				ensure!(
					(args.verify_bls_signatures) || args.trusted_signer.is_some(),
					// The client can't be executed in the trustless mode without BLS sigs
					// verification on Mainnet
					Error::<T>::TrustlessModeError,
				);
			}

			let finalized_execution_header_hash = args.finalized_execution_header.calculate_hash();
			ensure!(
				finalized_execution_header_hash ==
					args.finalized_beacon_header.execution_block_hash,
				// Invalid execution block
				Error::<T>::InvalidExecutionBlock,
			);

			let finalized_execution_header_info = ExecutionHeaderInfo {
				parent_hash: args.finalized_execution_header.parent_hash,
				block_number: args.finalized_execution_header.number,
				submitter: signer,
			};

			if let Some(account) = args.trusted_signer.clone() {
				TrustedSigner::<T>::put(account);
			}

			Paused::<T>::insert(typed_chain_id, false);
			ValidateUpdates::<T>::insert(typed_chain_id, args.validate_updates);
			VerifyBlsSignatures::<T>::insert(typed_chain_id, args.verify_bls_signatures);
			HashesGcThreshold::<T>::insert(typed_chain_id, args.hashes_gc_threshold);
			// Insert the first finalized execution block
			FinalizedExecutionBlocks::<T>::insert(
				typed_chain_id,
				args.finalized_execution_header.number,
				finalized_execution_header_hash,
			);
			FinalizedBeaconHeader::<T>::insert(typed_chain_id, args.finalized_beacon_header);
			FinalizedExecutionHeader::<T>::insert(
				typed_chain_id,
				finalized_execution_header_info.clone(),
			);
			CurrentSyncCommittee::<T>::insert(typed_chain_id, args.current_sync_committee);
			NextSyncCommittee::<T>::insert(typed_chain_id, args.next_sync_committee);
			ClientModeForChain::<T>::insert(typed_chain_id, ClientMode::SubmitLightClientUpdate);

			Self::deposit_event(Event::Init {
				typed_chain_id,
				header_info: finalized_execution_header_info,
			});

			Ok(().into())
		}

		#[pallet::weight({0})]
		#[pallet::call_index(2)]
		pub fn submit_beacon_chain_light_client_update(
			origin: OriginFor<T>,
			typed_chain_id: TypedChainId,
			light_client_update: LightClientUpdate,
		) -> DispatchResultWithPostInfo {
			let submitter = ensure_signed(origin)?;
			// Check if the update is allowed
			Self::is_light_client_update_allowed(&submitter, typed_chain_id)?;
			// Check if we are validating updates
			if Self::validate_updates(typed_chain_id) {
				// Validate the update
				Self::validate_light_client_update(typed_chain_id, &light_client_update)?;
			}

			Self::commit_light_client_update(typed_chain_id, light_client_update.clone())?;
			Self::deposit_event(Event::SubmitBeaconChainLightClientUpdate {
				typed_chain_id,
				submitter,
				beacon_block_header: light_client_update.attested_beacon_header,
			});
			Ok(().into())
		}

		#[pallet::weight({4})]
		#[pallet::call_index(4)]
		pub fn submit_execution_header(
			origin: OriginFor<T>,
			typed_chain_id: TypedChainId,
			block_header: BlockHeader,
		) -> DispatchResultWithPostInfo {
			let submitter = ensure_signed(origin)?;
			ensure!(
				ClientModeForChain::<T>::get(typed_chain_id) == Some(ClientMode::SubmitHeader),
				Error::<T>::InvalidClientMode
			);

			let block_hash = block_header.calculate_hash();
			let expected_block_hash = UnfinalizedTailExecutionHeader::<T>::get(typed_chain_id)
				.map(|header| header.parent_hash)
				.unwrap_or_else(|| {
					FinalizedBeaconHeader::<T>::get(typed_chain_id)
						.map(|header| header.execution_block_hash)
						.unwrap_or_default()
				});

			ensure!(block_hash == expected_block_hash, Error::<T>::BlockHashesDoNotMatch,);

			// Ensure that the block header is not already submitted then insert it.
			ensure!(
				!FinalizedExecutionBlocks::<T>::contains_key(typed_chain_id, block_header.number),
				Error::<T>::BlockAlreadySubmitted
			);
			FinalizedExecutionBlocks::<T>::insert(typed_chain_id, block_header.number, block_hash);

			let finalized_execution_header = FinalizedExecutionHeader::<T>::get(typed_chain_id)
				.ok_or(Error::<T>::FinalizedExecutionHeaderNotPresent)?;

			// Apply gc
			if let Some(diff_between_unfinalized_head_and_tail) =
				Self::get_diff_between_unfinalized_head_and_tail(typed_chain_id)
			{
				let header_number_to_remove = (finalized_execution_header.block_number +
					diff_between_unfinalized_head_and_tail)
					.saturating_sub(HashesGcThreshold::<T>::get(typed_chain_id));

				ensure!(
					header_number_to_remove < finalized_execution_header.block_number,
					Error::<T>::HashesGcThresholdInsufficient,
				);

				if header_number_to_remove > 0 {
					Self::gc_finalized_execution_blocks(typed_chain_id, header_number_to_remove);
				}
			}

			if block_header.number == finalized_execution_header.block_number + 1 {
				let finalized_execution_header_hash = FinalizedExecutionBlocks::<T>::get(
					typed_chain_id,
					finalized_execution_header.block_number,
				)
				.ok_or(Error::<T>::HeaderHashDoesNotExist)?;
				ensure!(
					block_header.parent_hash == finalized_execution_header_hash,
					Error::<T>::ChainCannotBeClosed,
				);

				let unfinalized_head_execution_header =
					UnfinalizedHeadExecutionHeader::<T>::get(typed_chain_id)
						.ok_or(Error::<T>::UnfinalizedHeaderNotPresent)?;
				frame_support::log::debug!(
					target: "light-client",
					"Current finalized block number: {:?}, New finalized block number: {:?}",
					finalized_execution_header.block_number,
					unfinalized_head_execution_header.block_number,
				);

				FinalizedExecutionHeader::<T>::insert(
					typed_chain_id,
					unfinalized_head_execution_header,
				);
				UnfinalizedTailExecutionHeader::<T>::remove(typed_chain_id);
				UnfinalizedHeadExecutionHeader::<T>::remove(typed_chain_id);
				ClientModeForChain::<T>::insert(
					typed_chain_id,
					ClientMode::SubmitLightClientUpdate,
				);
			} else {
				let block_info = ExecutionHeaderInfo {
					parent_hash: block_header.parent_hash,
					block_number: block_header.number,
					submitter,
				};

				if UnfinalizedHeadExecutionHeader::<T>::get(typed_chain_id).is_none() {
					UnfinalizedHeadExecutionHeader::<T>::insert(typed_chain_id, block_info.clone());
				}
				UnfinalizedTailExecutionHeader::<T>::insert(typed_chain_id, block_info);
			}

			frame_support::log::debug!(
				target: "light-client",
				"Submitted header number {:?}, hash {:#?}",
				block_header.number, block_hash
			);

			Self::deposit_event(Event::SubmitExecutionHeader {
				typed_chain_id,
				header_info: Box::new(block_header),
			});

			Ok(().into())
		}

		#[pallet::weight({5})]
		#[pallet::call_index(5)]
		pub fn update_trusted_signer(
			origin: OriginFor<T>,
			trusted_signer: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let origin = ensure_signed(origin)?;
			ensure!(TrustedSigner::<T>::get() == Some(origin), Error::<T>::NotTrustedSigner);
			TrustedSigner::<T>::put(trusted_signer.clone());

			Self::deposit_event(Event::UpdateTrustedSigner { trusted_signer });

			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// We consider the system initialized if any block is finalized for that chain.
	pub fn initialized(typed_chain_id: TypedChainId) -> bool {
		FinalizedBeaconHeader::<T>::contains_key(typed_chain_id)
	}

	/// Returns finalized execution block number
	pub fn last_block_number(typed_chain_id: TypedChainId) -> u64 {
		match Self::finalized_execution_header(typed_chain_id) {
			Some(header) => header.block_number,
			None => 0,
		}
	}

	/// Returns finalized execution block hash
	pub fn block_hash_safe(typed_chain_id: TypedChainId, block_number: u64) -> Option<H256> {
		match Self::finalized_execution_header(typed_chain_id) {
			Some(header) =>
				if header.block_number >= block_number {
					Self::finalized_execution_blocks(typed_chain_id, block_number)
				} else {
					None
				},
			None => None,
		}
	}

	/// Checks if the execution header is already submitted.
	pub fn is_known_execution_header(typed_chain_id: TypedChainId, block_number: u64) -> bool {
		Self::finalized_execution_blocks(typed_chain_id, block_number).is_some()
	}

	/// Get finalized beacon block root
	pub fn finalized_beacon_block_root(typed_chain_id: TypedChainId) -> H256 {
		match Self::finalized_beacon_block_header(typed_chain_id) {
			Some(header) => header.beacon_block_root,
			None => H256::default(),
		}
	}

	/// Returns finalized beacon block slot
	pub fn finalized_beacon_block_slot(typed_chain_id: TypedChainId) -> u64 {
		match Self::finalized_beacon_block_header(typed_chain_id) {
			Some(header) => header.header.slot,
			None => 0,
		}
	}

	/// Returns finalized beacon block header
	pub fn finalized_beacon_block_header(
		typed_chain_id: TypedChainId,
	) -> Option<ExtendedBeaconBlockHeader> {
		Self::finalized_beacon_header(typed_chain_id)
	}

	/// Get the current light client state
	pub fn get_light_client_state(typed_chain_id: TypedChainId) -> Option<LightClientState> {
		let finalized_beacon_header = Self::finalized_beacon_header(typed_chain_id);
		let current_sync_committee = Self::current_sync_committee(typed_chain_id);
		let next_sync_committee = Self::next_sync_committee(typed_chain_id);

		match (finalized_beacon_header, current_sync_committee, next_sync_committee) {
			(
				Some(finalized_beacon_header),
				Some(current_sync_committee),
				Some(next_sync_committee),
			) => Some(LightClientState {
				finalized_beacon_header,
				current_sync_committee,
				next_sync_committee,
			}),
			_ => None,
		}
	}

	pub fn get_client_mode(typed_chain_id: TypedChainId) -> Option<ClientMode> {
		Self::client_mode(typed_chain_id)
	}

	pub fn get_unfinalized_tail_block_number(typed_chain_id: TypedChainId) -> Option<u64> {
		Self::unfinalized_tail_execution_header(typed_chain_id).map(|header| header.block_number)
	}

	pub fn get_trusted_signer() -> Option<T::AccountId> {
		Self::trusted_signer()
	}

	/// Remove information about the headers that are at least as old as the given block number.
	/// This method could go out of gas if the client was not synced for a while, to fix that
	/// you need to increase the `hashes_gc_threshold` by calling `update_hashes_gc_threshold()`
	/// TODO: Run this on idle hooks possible? (@1xstj)
	pub fn gc_finalized_execution_blocks(typed_chain_id: TypedChainId, mut header_number: u64) {
		loop {
			if Self::finalized_execution_blocks(typed_chain_id, header_number).is_some() {
				FinalizedExecutionBlocks::<T>::remove(typed_chain_id, header_number);
				if header_number == 0 {
					break
				} else {
					header_number -= 1;
				}
			} else {
				break
			}
		}
	}

	pub fn is_light_client_update_allowed(
		submitter: &T::AccountId,
		typed_chain_id: TypedChainId,
	) -> Result<(), DispatchError> {
		ensure!(
			ClientModeForChain::<T>::get(typed_chain_id) ==
				Some(ClientMode::SubmitLightClientUpdate),
			Error::<T>::InvalidClientMode
		);
		ensure!(!Paused::<T>::get(typed_chain_id), Error::<T>::LightClientUpdateNotAllowed);
		if TrustedSigner::<T>::get().is_some() {
			ensure!(
				TrustedSigner::<T>::get() == Some(submitter.clone()),
				Error::<T>::NotTrustedSigner
			);
		}
		Ok(())
	}

	pub fn get_diff_between_unfinalized_head_and_tail(typed_chain_id: TypedChainId) -> Option<u64> {
		let head_block_number = match Self::unfinalized_head_execution_header(typed_chain_id) {
			Some(header) => header.block_number,
			None => return None,
		};

		let tail_block_number = match Self::unfinalized_tail_execution_header(typed_chain_id) {
			Some(header) => header.block_number,
			None => return None,
		};

		Some(head_block_number - tail_block_number)
	}

	pub fn validate_light_client_update(
		typed_chain_id: TypedChainId,
		update: &LightClientUpdate,
	) -> Result<(), DispatchError> {
		let finalized_beacon_header = Self::finalized_beacon_header(typed_chain_id)
			.ok_or(Error::<T>::LightClientUpdateNotAllowed)?;
		let finalized_period = compute_sync_committee_period(finalized_beacon_header.header.slot);
		Self::verify_finality_branch(update, finalized_period, finalized_beacon_header)?;

		// Verify sync committee has sufficient participants
		let sync_committee_bits =
			BitVec::<u8, Lsb0>::from_slice(&update.sync_aggregate.sync_committee_bits.0);
		let sync_committee_bits_sum: u64 = sync_committee_bits.count_ones().try_into().unwrap();

		ensure!(
			sync_committee_bits_sum >= MIN_SYNC_COMMITTEE_PARTICIPANTS,
			// Invalid sync committee bits sum
			Error::<T>::InvalidSyncCommitteeBitsSum
		);
		ensure!(
			sync_committee_bits_sum * 3 >= (sync_committee_bits.len() * 2).try_into().unwrap(),
			// Sync committee bits sum is less than 2/3 threshold, bits sum
			Error::<T>::SyncCommitteeBitsSumLessThanThreshold
		);

		if Self::verify_bls_sigs(typed_chain_id) {
			Self::verify_bls_signatures(
				typed_chain_id,
				update,
				sync_committee_bits,
				finalized_period,
			)?;
		}

		Ok(())
	}

	fn verify_finality_branch(
		update: &LightClientUpdate,
		finalized_period: u64,
		last_finalized_beacon_header: ExtendedBeaconBlockHeader,
	) -> Result<(), DispatchError> {
		// The active header will always be the finalized header because we don't accept updates
		// without the finality update.
		let active_header = &update.finality_update.header_update.beacon_header;
		ensure!(
			active_header.slot > last_finalized_beacon_header.header.slot,
			Error::<T>::ActiveHeaderSlotLessThanFinalizedSlot
		);

		ensure!(
			update.attested_beacon_header.slot >=
				update.finality_update.header_update.beacon_header.slot,
			Error::<T>::UpdateHeaderSlotLessThanFinalizedHeaderSlot
		);

		ensure!(
			update.signature_slot > update.attested_beacon_header.slot,
			Error::<T>::UpdateSignatureSlotLessThanAttestedHeaderSlot
		);

		let update_period = compute_sync_committee_period(active_header.slot);
		ensure!(
			update_period == finalized_period || update_period == finalized_period + 1,
			Error::<T>::InvalidUpdatePeriod
		);

		// Verify that the `finality_branch`, confirms `finalized_header`
		// to match the finalized checkpoint root saved in the state of `attested_header`.
		let branch = convert_branch(&update.finality_update.finality_branch);
		ensure!(
			merkle_proof::verify_merkle_proof(
				update.finality_update.header_update.beacon_header.tree_hash_root(),
				&branch,
				FINALITY_TREE_DEPTH.try_into().unwrap(),
				FINALITY_TREE_INDEX.try_into().unwrap(),
				update.attested_beacon_header.state_root.0
			),
			Error::<T>::InvalidFinalityProof
		);
		ensure!(
			validate_beacon_block_header_update(&update.finality_update.header_update),
			Error::<T>::InvalidExecutionBlockHashProof
		);

		// Verify that the `next_sync_committee`, if present, actually is the next sync committee
		// saved in the state of the `active_header`
		if update_period != finalized_period {
			ensure!(
				update.sync_committee_update.is_some(),
				// The next sync committee should be present
				Error::<T>::SyncCommitteeUpdateNotPresent
			);
			let sync_committee_update = update.sync_committee_update.as_ref().unwrap();
			let branch = convert_branch(&sync_committee_update.next_sync_committee_branch);
			ensure!(
				merkle_proof::verify_merkle_proof(
					sync_committee_update.next_sync_committee.tree_hash_root(),
					&branch,
					SYNC_COMMITTEE_TREE_DEPTH.try_into().unwrap(),
					SYNC_COMMITTEE_TREE_INDEX.try_into().unwrap(),
					update.attested_beacon_header.state_root.0
				),
				// Invalid next sync committee proof
				Error::<T>::InvalidNextSyncCommitteeProof
			);
		}

		Ok(())
	}

	pub fn verify_bls_signatures(
		typed_chain_id: TypedChainId,
		update: &LightClientUpdate,
		sync_committee_bits: BitVec<u8>,
		finalized_period: u64,
	) -> Result<(), DispatchError> {
		let signature_period = compute_sync_committee_period(update.signature_slot);
		// Verify signature period does not skip a sync committee period
		// The acceptable signature periods are `signature_period`, `signature_period + 1`
		ensure!(
			signature_period == finalized_period || signature_period == finalized_period + 1,
			Error::<T>::InvalidSignaturePeriod
		);
		// Verify sync committee aggregate signature
		let sync_committee = if signature_period == finalized_period {
			ensure!(
				Self::current_sync_committee(typed_chain_id).is_some(),
				Error::<T>::CurrentSyncCommitteeNotSet
			);
			Self::current_sync_committee(typed_chain_id).unwrap()
		} else {
			ensure!(
				Self::next_sync_committee(typed_chain_id).is_some(),
				Error::<T>::NextSyncCommitteeNotSet
			);
			Self::next_sync_committee(typed_chain_id).unwrap()
		};

		let participant_pubkeys =
			get_participant_pubkeys(sync_committee.pubkeys.0.as_slice(), &sync_committee_bits);
		ensure!(
			Self::network_config_for_chain(typed_chain_id).is_some(),
			Error::<T>::InvalidNetworkConfig
		);
		let network_config = Self::network_config_for_chain(typed_chain_id).unwrap();
		let maybe_fork_version = network_config.compute_fork_version_by_slot(update.signature_slot);
		ensure!(
			maybe_fork_version.is_some(),
			// The fork version should be present
			Error::<T>::InvalidNetworkConfig
		);
		let domain = compute_domain(
			DOMAIN_SYNC_COMMITTEE,
			maybe_fork_version.unwrap(),
			H256::from(network_config.genesis_validators_root),
		);
		let signing_root =
			compute_signing_root(H256(update.attested_beacon_header.tree_hash_root()), domain);

		let aggregate_signature =
			bls::AggregateSignature::deserialize(&update.sync_aggregate.sync_committee_signature.0)
				.unwrap();
		let pubkeys: Vec<bls::PublicKey> = participant_pubkeys
			.into_iter()
			.map(|x| bls::PublicKey::deserialize(&x.0).unwrap())
			.collect();
		ensure!(
			aggregate_signature
				.fast_aggregate_verify(signing_root.0, &pubkeys.iter().collect::<Vec<_>>()),
			Error::<T>::InvalidBlsSignature
		);

		Ok(())
	}

	fn commit_light_client_update(
		typed_chain_id: TypedChainId,
		update: LightClientUpdate,
	) -> Result<(), DispatchError> {
		let finalized_beacon_header = Self::finalized_beacon_block_header(typed_chain_id)
			.ok_or(Error::<T>::FinalizedBeaconHeaderNotPresent)?;
		// Update finalized header
		let finalized_header_update = update.finality_update.header_update;
		let finalized_period = compute_sync_committee_period(finalized_beacon_header.header.slot);
		let update_period =
			compute_sync_committee_period(finalized_header_update.beacon_header.slot);

		if update_period == finalized_period + 1 {
			let maybe_next_sync_committee = NextSyncCommittee::<T>::get(typed_chain_id);
			ensure!(
				maybe_next_sync_committee.is_some(),
				// The next sync committee should be present
				Error::<T>::NextSyncCommitteeNotPresent
			);
			let next_sync_committee = maybe_next_sync_committee.unwrap();
			CurrentSyncCommittee::<T>::insert(typed_chain_id, next_sync_committee);

			ensure!(
				update.sync_committee_update.is_some(),
				// The sync committee update should be present
				Error::<T>::SyncCommitteeUpdateNotPresent
			);
			let next_sync_committee = update.sync_committee_update.unwrap().next_sync_committee;
			NextSyncCommittee::<T>::insert(typed_chain_id, next_sync_committee);
		}

		frame_support::log::debug!(
			target: "light-client",
			"Current finalized slot: {:?}, New finalized slot: {:?}",
			finalized_beacon_header.header.slot,
			finalized_header_update.beacon_header.slot
		);

		let extended_beacon_block_header: ExtendedBeaconBlockHeader =
			finalized_header_update.into();
		FinalizedBeaconHeader::<T>::insert(typed_chain_id, extended_beacon_block_header);
		ClientModeForChain::<T>::insert(typed_chain_id, ClientMode::SubmitHeader);
		Ok(())
	}

	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}
}

impl<T: Config> VerifyBlockHeaderExists for Pallet<T> {
	fn verify_block_header_exists(
		header: BlockHeader,
		typed_chain_id: TypedChainId,
	) -> Result<bool, DispatchError> {
		let block_number = header.number;
		ensure!(header.hash.is_some(), Error::<T>::HeaderHashDoesNotExist);
		let block_hash = header.hash.unwrap();

		let block_hash_from_storage =
			FinalizedExecutionBlocks::<T>::get(typed_chain_id, block_number);

		ensure!(block_hash_from_storage.is_some(), Error::<T>::HeaderHashDoesNotExist);
		ensure!(block_hash_from_storage.unwrap() == block_hash, Error::<T>::BlockHashesDoNotMatch);

		Ok(BlockHeader::calculate_hash(&header) == block_hash)
	}
}
