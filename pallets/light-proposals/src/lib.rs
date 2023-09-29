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
use scale_info::TypeInfo;
pub use pallet::*;

use bitvec::prelude::{BitVec, Lsb0};

use frame_support::{sp_runtime::traits::AccountIdConversion, traits::Currency};

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

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
		Blake2_128Concat
	};
	use frame_support::dispatch::fmt::Debug;
	use frame_system::pallet_prelude::*;
	use webb_light_client_primitives::{types::LightProposalInput, traits::LightClientHandler};

	pub type LightProposalInputOf<T> = LightProposalInput<<T as Config>::MaxProofSize>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	/// The module configuration trait.
	pub trait Config: frame_system::Config + pallet_balances::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Currency: Currency<Self::AccountId>;

		type LightClient: LightClientHandler;

		type MaxProofSize: Get<u32> + TypeInfo + Clone + Debug + PartialEq + Eq;
	}

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

	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().reads_writes(1,1))]
		pub fn submit_proposal(
			origin: OriginFor<T>,
			proposal: LightProposalInputOf<T>,
		) -> DispatchResultWithPostInfo {
			// // generate the block hash
			// let block_hash = Self::generate_block_hash(proposal.block_header);

			// // validate the block hash against the eth-light-client storage
			// ensure!(Self::is_block_hash_valid(block_hash), Error::<T>::InvalidBlock);

			// // validate the merkle proofs
			// ensure!(Self::validate_merkle_proof(proposal.proof), Error::<T>::InvalidProof);

			// // prepare the proposal
			// Self::prepare_proposal(proposal);

			Ok(().into())
		}
	}
}
