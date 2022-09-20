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
//!
//! A module for storing block headers from the Ethereum 2.0 Beacon Chain and
//! verifying their validity. It should also expose an API that allows one to
//! verify the state of the Ethereum 2.0 Beacon Chain at a given block header.
//!
//! ## Overview
//!
//! The Eth2 Light Client module provides functionality maintaing and storing
//! metadata about the Eth2 Beacon Chain.
//!
//! The supported dispatchable functions are documented in the [`Call`] enum.
//!
//! ### Terminology
//!
//! ### Goals
//!
//! ## Interface
//!
//! ## Related Modules
//!
//! * [`System`](../frame_system/index.html)
//! * [`Support`](../frame_support/index.html)

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use eth_types::{
    eth2::{
        Epoch, ExtendedBeaconBlockHeader, ForkVersion, LightClientState, LightClientUpdate, Slot,
        SyncCommittee,
    },
    BlockHeader, H256,
};
use frame_support::pallet_prelude::{ensure, DispatchError};
use frame_support::{traits::Get, PalletId};
use sp_runtime::traits::Saturating;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::{convert::TryInto, prelude::*};
use tree_hash::TreeHash;
use types::{ExecutionHeaderInfo, InitInput};
use webb_proposals::TypedChainId;

pub use pallet::*;

use crate::consensus::{
    compute_domain, compute_epoch_at_slot, compute_signing_root, compute_sync_committee_period,
    convert_branch, get_participant_pubkeys, validate_beacon_block_header_update,
    DOMAIN_SYNC_COMMITTEE, FINALITY_TREE_DEPTH, FINALITY_TREE_INDEX,
    MIN_SYNC_COMMITTEE_PARTICIPANTS, SYNC_COMMITTEE_TREE_DEPTH, SYNC_COMMITTEE_TREE_INDEX,
};
use bitvec::prelude::BitVec;
use bitvec::prelude::Lsb0;

use frame_support::traits::{Currency, ExistenceRequirement};
use sp_runtime::traits::AccountIdConversion;
type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod test_utils;

mod consensus;
mod traits;
mod types;

pub use traits::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        dispatch::DispatchResultWithPostInfo,
        pallet_prelude::{OptionQuery, *},
        Blake2_128Concat,
    };
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    /// The module configuration trait.
    pub trait Config: frame_system::Config + pallet_balances::Config {
        /// The overarching event type.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        #[pallet::constant]
        type PalletId: Get<PalletId>;

        type Currency: Currency<Self::AccountId>;

        type StoragePricePerByte: Get<BalanceOf<Self>>;
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub networks: Vec<(TypedChainId, [u8; 32], ForkVersion, u64)>,
        pub phantom: PhantomData<T>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                networks: vec![],
                phantom: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            for n in self.networks.clone() {
                match n.0 {
                    TypedChainId::Evm(_) => {}
                    _ => {
                        panic!("Unsupported network type, ETH2 chains are only supported on EVM networks");
                    }
                }
                GenesisValidatorsRoot::<T>::insert(n.0, n.1);
                BellatrixForkVersion::<T>::insert(n.0, n.2);
                BellatrixForkEpoch::<T>::insert(n.0, n.3);
            }
        }
    }

    /************* STORAGE *************/

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

    /// Hashes of the finalized execution blocks mapped to their numbers. Stores up to `hashes_gc_threshold` entries.
    /// Execution block number -> execution block hash
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

    /// All unfinalized execution blocks' headers hashes mapped to their `HeaderInfo`.
    /// Execution block hash -> ExecutionHeaderInfo object
    #[pallet::storage]
    #[pallet::getter(fn unfinalized_headers)]
    pub(super) type UnfinalizedHeaders<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        TypedChainId,
        Blake2_128Concat,
        H256,
        types::ExecutionHeaderInfo<T::AccountId>,
        OptionQuery,
    >;

    /// `AccountId`s mapped to their number of submitted headers.
    /// Submitter account -> Num of submitted headers
    #[pallet::storage]
    #[pallet::getter(fn submitters)]
    pub(super) type Submitters<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        TypedChainId,
        Blake2_128Concat,
        T::AccountId,
        u32,
        OptionQuery,
    >;

    /// Max number of unfinalized blocks allowed to be stored by one submitter account
    /// This value should be at least 32 blocks (1 epoch), but the recommended value is 1024 (32 epochs
    #[pallet::storage]
    #[pallet::getter(fn max_unfinalized_blocks_per_submitter)]
    pub(super) type MaxUnfinalizedBlocksPerSubmitter<T: Config> =
        StorageMap<_, Blake2_128Concat, TypedChainId, u32, ValueQuery>;

    /// The minimum balance that should be attached to register a new submitter account
    #[pallet::storage]
    #[pallet::getter(fn min_submitter_balance)]
    pub(super) type MinSubmitterBalance<T: Config> =
        StorageMap<_, Blake2_128Concat, TypedChainId, BalanceOf<T>, ValueQuery>;

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
        types::ExecutionHeaderInfo<T::AccountId>,
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
    #[pallet::getter(fn genesis_validators_root)]
    pub(super) type GenesisValidatorsRoot<T: Config> =
        StorageMap<_, Blake2_128Concat, TypedChainId, [u8; 32], OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn bellatrix_fork_version)]
    pub(super) type BellatrixForkVersion<T: Config> =
        StorageMap<_, Blake2_128Concat, TypedChainId, ForkVersion, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn bellatrix_fork_epoch)]
    pub(super) type BellatrixForkEpoch<T: Config> =
        StorageMap<_, Blake2_128Concat, TypedChainId, u64, OptionQuery>;

    /************* STORAGE *************/

    #[pallet::event]
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {
        /// For attempting to register
        SubmitterAlreadyRegistered,
        /// For attempting to unregister
        SubmitterNotRegistered,
        /// For attempting to unregister
        SubmitterHasUsedStorage,
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
        /// The client can't be executed in the trustless mode without BLS sigs verification on Mainnet
        TrustlessModeError,
        InvalidSyncCommitteeBitsSum,
        SyncCommitteeBitsSumLessThanThreshold,
        ForkVersionNotFound,
        ForkEpochNotFound,
        GenesisValidatorsRootNotFound,
        InvalidBlsSignature,
        InvalidExecutionBlock,
        ActiveHeaderSlotNumberLessThanFinalizedSlot,
        InvalidUpdatePeriod,
        InvalidFinalityProof,
        InvalidExecutionBlockHashProof,
        NextSyncCommitteeNotPresent,
        InvalidNextSyncCommitteeProof,
        FinalizedExecutionHeaderNotPresent,
        FinalizedBeaconHeaderNotPresent,
        UnfinalizedHeaderNotPresent,
        SyncCommitteeUpdateNotPresent,
        SubmitterNotPresent,
        SubmitterExhaustedLimit,
        /// Header hash does not exist
        HeaderHashDoesNotExist,
        /// When storge block hash and submitted header hash don't match
        BlockHashesDoNotMatch,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(0)]
        pub fn init(
            origin: OriginFor<T>,
            typed_chain_id: TypedChainId,
            args: InitInput<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            let signer = ensure_signed(origin)?;

            let min_storage_balance_for_submitter =
                Self::calculate_min_storage_balance_for_submitter(
                    args.max_submitted_blocks_by_account,
                );
            if typed_chain_id == TypedChainId::Evm(1) {
                ensure!(
                    args.validate_updates,
                    // The updates validation can't be disabled for mainnet
                    Error::<T>::ValidateUpdatesParameterError,
                );

                ensure!(
                    (args.verify_bls_signatures) || args.trusted_signer.is_some(),
                    // The client can't be executed in the trustless mode without BLS sigs verification on Mainnet
                    Error::<T>::TrustlessModeError,
                );
            }

            ensure!(
                args.finalized_execution_header.calculate_hash()
                    == args.finalized_beacon_header.execution_block_hash,
                // Invalid execution block
                Error::<T>::InvalidExecutionBlock,
            );

            let finalized_execution_header_info = types::ExecutionHeaderInfo {
                parent_hash: args.finalized_execution_header.parent_hash,
                block_number: args.finalized_execution_header.number,
                submitter: signer.clone(),
            };

            if let Some(account) = args.trusted_signer.clone() {
                TrustedSigner::<T>::put(account);
            }

            Paused::<T>::insert(typed_chain_id, false);
            ValidateUpdates::<T>::insert(typed_chain_id, args.validate_updates);
            VerifyBlsSignatures::<T>::insert(typed_chain_id, args.verify_bls_signatures);
            HashesGcThreshold::<T>::insert(typed_chain_id, args.hashes_gc_threshold);
            MaxUnfinalizedBlocksPerSubmitter::<T>::insert(
                typed_chain_id,
                args.max_submitted_blocks_by_account,
            );
            MinSubmitterBalance::<T>::insert(typed_chain_id, min_storage_balance_for_submitter);
            FinalizedBeaconHeader::<T>::insert(typed_chain_id, args.finalized_beacon_header);
            FinalizedExecutionHeader::<T>::insert(typed_chain_id, finalized_execution_header_info);
            CurrentSyncCommittee::<T>::insert(typed_chain_id, args.current_sync_committee);
            NextSyncCommittee::<T>::insert(typed_chain_id, args.next_sync_committee);
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn register_submitter(
            origin: OriginFor<T>,
            typed_chain_id: TypedChainId,
        ) -> DispatchResultWithPostInfo {
            let submitter = ensure_signed(origin)?;
            ensure!(
                !Submitters::<T>::contains_key(typed_chain_id, &submitter),
                Error::<T>::SubmitterAlreadyRegistered
            );
            // Transfer the deposit amount to the pallet
            let deposit = MinSubmitterBalance::<T>::get(typed_chain_id);
            T::Currency::transfer(
                &submitter,
                &Self::account_id(),
                deposit,
                ExistenceRequirement::KeepAlive,
            )?;
            // Register the submitter
            Submitters::<T>::insert(typed_chain_id, submitter, 0);
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn unregister_submitter(
            origin: OriginFor<T>,
            typed_chain_id: TypedChainId,
        ) -> DispatchResultWithPostInfo {
            let submitter = ensure_signed(origin)?;
            ensure!(
                Submitters::<T>::contains_key(typed_chain_id, &submitter),
                Error::<T>::SubmitterNotRegistered
            );
            let submitter_block_count = Submitters::<T>::get(typed_chain_id, &submitter).unwrap();
            ensure!(
                submitter_block_count == 0u32,
                Error::<T>::SubmitterHasUsedStorage
            );
            // Transfer the deposit amount back to the submitter
            let deposit = MinSubmitterBalance::<T>::get(typed_chain_id);
            T::Currency::transfer(
                &Self::account_id(),
                &submitter,
                deposit,
                ExistenceRequirement::KeepAlive,
            )?;
            Ok(().into())
        }

        #[pallet::weight(0)]
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

            Self::commit_light_client_update(typed_chain_id, light_client_update)?;
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn submit_execution_header(
            origin: OriginFor<T>,
            typed_chain_id: TypedChainId,
            block_header: BlockHeader,
        ) -> DispatchResultWithPostInfo {
            let submitter = ensure_signed(origin)?;

            ensure!(
                Self::finalized_beacon_header(typed_chain_id).is_some(),
                Error::<T>::LightClientUpdateNotAllowed
            );
            let finalized_beacon_header = Self::finalized_beacon_header(typed_chain_id).unwrap();
            log::debug!("Submitted header number {}", block_header.number);
            if finalized_beacon_header.execution_block_hash != block_header.parent_hash {
                ensure!(
                    UnfinalizedHeaders::<T>::get(typed_chain_id, &block_header.parent_hash)
                        .is_some(),
                    Error::<T>::UnknownParentHeader,
                );
            }

            Self::update_submitter(typed_chain_id, &submitter, 1)?;
            let block_hash = block_header.calculate_hash();
            log::debug!("Submitted header hash {:?}", block_hash);

            let block_info = ExecutionHeaderInfo {
                parent_hash: block_header.parent_hash,
                block_number: block_header.number,
                submitter,
            };
            ensure!(
                UnfinalizedHeaders::<T>::get(typed_chain_id, &block_hash).is_none(),
                // The block {} already submitted!
                // &block_hash
                Error::<T>::BlockAlreadySubmitted,
            );
            UnfinalizedHeaders::<T>::insert(typed_chain_id, &block_hash, &block_info);
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn update_trusted_signer(
            origin: OriginFor<T>,
            trusted_signer: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let origin = ensure_signed(origin)?;
            ensure!(
                TrustedSigner::<T>::get() == Some(origin),
                Error::<T>::NotTrustedSigner
            );
            TrustedSigner::<T>::put(trusted_signer);
            Ok(().into())
        }
    }
}

impl<T: Config> Pallet<T> {
    pub fn calculate_min_storage_balance_for_submitter(
        max_submitted_blocks_by_account: u32,
    ) -> BalanceOf<T> {
        const STORAGE_BYTES_PER_BLOCK: u32 = 105; // prefix: 3B + key: 32B + HeaderInfo 70B
        const STORAGE_BYTES_PER_ACCOUNT: u32 = 39; // prefix: 3B + account_id: 32B + counter 4B
        let storage_bytes_per_account = (STORAGE_BYTES_PER_BLOCK
            * max_submitted_blocks_by_account as u32)
            + STORAGE_BYTES_PER_ACCOUNT;
        T::StoragePricePerByte::get().saturating_mul(storage_bytes_per_account.into())
    }

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
        Self::finalized_execution_blocks(typed_chain_id, block_number)
    }

    /// Checks if the execution header is already submitted.
    pub fn is_known_execution_header(typed_chain_id: TypedChainId, hash: H256) -> bool {
        Self::unfinalized_headers(typed_chain_id, hash).is_some()
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

    /// Returns the minimum balance that should be attached to register a new submitter account
    pub fn min_storage_balance_for_submitter(typed_chain_id: TypedChainId) -> BalanceOf<T> {
        Self::min_submitter_balance(typed_chain_id)
    }

    /// Get the current light client state
    pub fn get_light_client_state(typed_chain_id: TypedChainId) -> Option<LightClientState> {
        let finalized_beacon_header = Self::finalized_beacon_header(typed_chain_id);
        let current_sync_committee = Self::current_sync_committee(typed_chain_id);
        let next_sync_committee = Self::next_sync_committee(typed_chain_id);

        match (
            finalized_beacon_header,
            current_sync_committee,
            next_sync_committee,
        ) {
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

    pub fn get_trusted_signer(&self) -> Option<T::AccountId> {
        Self::trusted_signer()
    }

    pub fn is_light_client_update_allowed(
        submitter: &T::AccountId,
        typed_chain_id: TypedChainId,
    ) -> Result<(), DispatchError> {
        ensure!(
            Paused::<T>::get(typed_chain_id),
            Error::<T>::LightClientUpdateNotAllowed
        );
        ensure!(
            TrustedSigner::<T>::get() == Some(submitter.clone()),
            Error::<T>::NotTrustedSigner
        );
        Ok(())
    }

    pub fn validate_light_client_update(
        typed_chain_id: TypedChainId,
        update: &LightClientUpdate,
    ) -> Result<(), DispatchError> {
        ensure!(
            Self::finalized_beacon_header(typed_chain_id).is_some(),
            Error::<T>::LightClientUpdateNotAllowed
        );
        let finalized_beacon_header = Self::finalized_beacon_header(typed_chain_id).unwrap();
        // TODO: Generalize this over other networks once we know their parameters. This
        // is currently configured for the Ethereum 2.0 mainnet most likely (taken from rainbow-bridge).
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

    pub fn verify_bls_signatures(
        typed_chain_id: TypedChainId,
        update: &LightClientUpdate,
        sync_committee_bits: BitVec<u8>,
        finalized_period: u64,
    ) -> Result<(), DispatchError> {
        let signature_period = compute_sync_committee_period(update.signature_slot);
        // Verify sync committee aggregate signature
        // TODO: Ensure these storage values exist before unwrapping
        let sync_committee = if signature_period == finalized_period {
            Self::current_sync_committee(typed_chain_id).unwrap()
        } else {
            Self::next_sync_committee(typed_chain_id).unwrap()
        };

        let participant_pubkeys =
            get_participant_pubkeys(&sync_committee.pubkeys.0, &sync_committee_bits);
        ensure!(
            Self::bellatrix_fork_version(typed_chain_id).is_some(),
            Error::<T>::ForkVersionNotFound
        );
        ensure!(
            Self::bellatrix_fork_epoch(typed_chain_id).is_some(),
            Error::<T>::ForkEpochNotFound
        );
        ensure!(
            Self::genesis_validators_root(typed_chain_id).is_some(),
            Error::<T>::GenesisValidatorsRootNotFound
        );
        let fork_version = Self::bellatrix_fork_version(typed_chain_id).unwrap();
        let genesis_validators_root = Self::genesis_validators_root(typed_chain_id).unwrap();
        let domain = compute_domain(
            DOMAIN_SYNC_COMMITTEE,
            fork_version,
            genesis_validators_root.into(),
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
            // Failed to verify the bls signature
            Error::<T>::InvalidBlsSignature
        );

        Ok(())
    }

    fn verify_finality_branch(
        update: &LightClientUpdate,
        finalized_period: u64,
        finalized_beacon_header: ExtendedBeaconBlockHeader,
    ) -> Result<(), DispatchError> {
        // The active header will always be the finalized header because we don't accept updates without the finality update.
        let active_header = &update.finality_update.header_update.beacon_header;
        ensure!(
            active_header.slot > finalized_beacon_header.header.slot,
            // The active header slot number should be higher than the finalized slot
            Error::<T>::ActiveHeaderSlotNumberLessThanFinalizedSlot
        );

        let update_period = compute_sync_committee_period(active_header.slot);
        ensure!(
            update_period == finalized_period || update_period == finalized_period + 1,
            // The acceptable update periods are '{}' and '{}' but got {}
            // finalized_period,
            // finalized_period + 1,
            // update_period
            Error::<T>::InvalidUpdatePeriod
        );

        // Verify that the `finality_branch`, confirms `finalized_header`
        // to match the finalized checkpoint root saved in the state of `attested_header`.
        let branch = convert_branch(&update.finality_update.finality_branch);
        ensure!(
            merkle_proof::verify_merkle_proof(
                update
                    .finality_update
                    .header_update
                    .beacon_header
                    .tree_hash_root(),
                &branch,
                FINALITY_TREE_DEPTH.try_into().unwrap(),
                FINALITY_TREE_INDEX.try_into().unwrap(),
                update.attested_beacon_header.state_root.0
            ),
            // Invalid finality proof
            Error::<T>::InvalidFinalityProof
        );
        ensure!(
            validate_beacon_block_header_update(&update.finality_update.header_update),
            // Invalid execution block hash proof
            Error::<T>::InvalidExecutionBlockHashProof
        );

        // Verify that the `next_sync_committee`, if present, actually is the next sync committee saved in the
        // state of the `active_header`
        if update_period != finalized_period {
            ensure!(
                update.sync_committee_update.is_some(),
                // The next sync committee should be present
                Error::<T>::NextSyncCommitteeNotPresent
            );
            let sync_committee_update = update.sync_committee_update.as_ref().unwrap();
            let branch = convert_branch(&sync_committee_update.next_sync_committee_branch);
            ensure!(
                merkle_proof::verify_merkle_proof(
                    sync_committee_update.next_sync_committee.tree_hash_root(),
                    &branch,
                    SYNC_COMMITTEE_TREE_DEPTH.try_into().unwrap(),
                    SYNC_COMMITTEE_TREE_INDEX.try_into().unwrap(),
                    active_header.state_root.0
                ),
                // Invalid next sync committee proof
                Error::<T>::InvalidNextSyncCommitteeProof
            );
        }

        Ok(())
    }

    pub fn compute_fork_version(
        fork_epoch: Epoch,
        epoch: Epoch,
        fork_version: ForkVersion,
    ) -> Option<ForkVersion> {
        if epoch >= fork_epoch {
            return Some(fork_version);
        }

        None
    }

    pub fn compute_fork_version_by_slot(
        fork_epoch: Epoch,
        slot: Slot,
        fork_version: ForkVersion,
    ) -> Option<ForkVersion> {
        Self::compute_fork_version(fork_epoch, compute_epoch_at_slot(slot), fork_version)
    }

    fn update_finalized_header(
        typed_chain_id: TypedChainId,
        finalized_header: ExtendedBeaconBlockHeader,
    ) -> Result<(), DispatchError> {
        let maybe_finalized_execution_header_info =
            Self::unfinalized_headers(typed_chain_id, &finalized_header.execution_block_hash);
        ensure!(
            maybe_finalized_execution_header_info.is_some(),
            // The finalized execution header should be present
            Error::<T>::FinalizedExecutionHeaderNotPresent
        );
        let finalized_execution_header_info = maybe_finalized_execution_header_info.unwrap();
        let maybe_current_finalized_beacon_header =
            Self::finalized_beacon_block_header(typed_chain_id);
        ensure!(
            maybe_current_finalized_beacon_header.is_some(),
            // The finalized beacon header should be present
            Error::<T>::FinalizedBeaconHeaderNotPresent
        );
        let current_finalized_beacon_header = maybe_current_finalized_beacon_header.unwrap();
        log::debug!(
            "Current finalized slot: {}, New finalized slot: {}",
            current_finalized_beacon_header.header.slot,
            finalized_header.header.slot
        );

        let mut cursor_header = finalized_execution_header_info.clone();
        let mut cursor_header_hash = finalized_header.execution_block_hash;

        let mut submitters_update: BTreeMap<T::AccountId, u32> = BTreeMap::new();
        loop {
            let num_of_removed_headers = submitters_update
                .get(&cursor_header.submitter)
                .unwrap_or(&0);
            submitters_update.insert(cursor_header.submitter, num_of_removed_headers + 1);

            UnfinalizedHeaders::<T>::remove(typed_chain_id, &cursor_header_hash);
            FinalizedExecutionBlocks::<T>::insert(
                typed_chain_id,
                &cursor_header.block_number,
                cursor_header_hash,
            );

            if cursor_header.parent_hash == current_finalized_beacon_header.execution_block_hash {
                break;
            }

            cursor_header_hash = cursor_header.parent_hash;
            ensure!(
                Self::unfinalized_headers(typed_chain_id, &cursor_header_hash).is_some(),
                // The unfinalized header should be present
                Error::<T>::UnfinalizedHeaderNotPresent
            );
            cursor_header =
                Self::unfinalized_headers(typed_chain_id, &cursor_header.parent_hash).unwrap();
        }
        FinalizedBeaconHeader::<T>::insert(typed_chain_id, finalized_header);
        FinalizedExecutionHeader::<T>::insert(
            typed_chain_id,
            finalized_execution_header_info.clone(),
        );

        for (submitter, num_of_removed_headers) in &submitters_update {
            Self::update_submitter(typed_chain_id, submitter, -(*num_of_removed_headers as i64))?;
        }

        log::debug!("Finish update finalized header..");
        if finalized_execution_header_info.block_number > Self::hashes_gc_threshold(typed_chain_id)
        {
            Self::gc_headers(
                typed_chain_id,
                finalized_execution_header_info.block_number
                    - Self::hashes_gc_threshold(typed_chain_id),
            );
        }

        Ok(())
    }

    fn commit_light_client_update(
        typed_chain_id: TypedChainId,
        update: LightClientUpdate,
    ) -> Result<(), DispatchError> {
        let maybe_finalized_beacon_header = Self::finalized_beacon_block_header(typed_chain_id);
        ensure!(
            maybe_finalized_beacon_header.is_some(),
            // The finalized beacon header should be present
            Error::<T>::FinalizedBeaconHeaderNotPresent
        );
        let finalized_beacon_header = maybe_finalized_beacon_header.unwrap();
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

        Self::update_finalized_header(typed_chain_id, finalized_header_update.into())?;
        Ok(())
    }

    /// Remove information about the headers that are at least as old as the given block number.
    fn gc_headers(typed_chain_id: TypedChainId, mut header_number: u64) {
        loop {
            if FinalizedExecutionBlocks::<T>::contains_key(typed_chain_id, &header_number) {
                FinalizedExecutionBlocks::<T>::remove(typed_chain_id, &header_number);

                if header_number == 0 {
                    break;
                } else {
                    header_number -= 1;
                }
            } else {
                break;
            }
        }
    }

    fn update_submitter(
        typed_chain_id: TypedChainId,
        submitter: &T::AccountId,
        value: i64,
    ) -> Result<(), DispatchError> {
        ensure!(
            Self::submitters(typed_chain_id, submitter).is_some(),
            // The submitter should be present
            Error::<T>::SubmitterNotPresent
        );
        let mut num_of_submitted_headers: i64 =
            Self::submitters(typed_chain_id, submitter).unwrap().into();
        num_of_submitted_headers += value;

        ensure!(
            num_of_submitted_headers
                <= Self::max_unfinalized_blocks_per_submitter(typed_chain_id).into(),
            // The submitter {} exhausted the limit of blocks ({})",
            // &submitter, self.max_submitted_blocks_by_account
            Error::<T>::SubmitterExhaustedLimit
        );

        Submitters::<T>::insert(typed_chain_id, submitter, num_of_submitted_headers as u32);
        Ok(())
    }

    pub fn account_id() -> T::AccountId {
        T::PalletId::get().into_account_truncating()
    }
}

impl<T: Config> VerifyBlockHeaderExists for Pallet<T> {
    fn verify_block_header_exists(header: BlockHeader, typed_chain_id: TypedChainId) {
        let block_number = header.number;
        ensure!(header.hash.is_some(), Error::<T>::HeaderHashDoesNotExist);
        let block_hash = header.hash.unwrap();

        let block_hash_from_storage =
            FinalizedExecutionBlocks::<T>::get(typed_chain_id, block_number);

        ensure!(
            block_hash_from_storage.is_some(),
            Error::<T>::HeaderHashDoesNotExist
        );
        ensure!(
            block_hash_from_storage.unwrap() == block_hash,
            Error::<T>::BlockHashesDoNotMatch
        );

        BlockHeader::calculate_hash(&header) == block_hash
    }
}
