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
#![allow(unused)]

use dkg_runtime_primitives::{
	FunctionSignature, Proposal, ProposalHandlerTrait, ProposalHeader, ProposalKind, ResourceId,
};
use frame_support::{pallet_prelude::DispatchError, traits::Get};
pub use pallet::*;
use scale_info::TypeInfo;
use sp_std::{convert::TryInto, prelude::*};

use webb::evm::{
	contract::protocol_solidity::variable_anchor::v_anchor_contract, ethers::contract::EthCall,
};
use webb_light_client_primitives::{types::LightProposalInput, LEAF_INDEX_KEY, MERKLE_ROOT_KEY};
use webb_proposals::{evm::AnchorUpdateProposal, Nonce, TypedChainId};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	use dkg_runtime_primitives::ProposalHandlerTrait;

	use frame_support::{
		dispatch::{fmt::Debug, DispatchResultWithPostInfo},
		pallet_prelude::{ValueQuery, *},
	};
	use frame_system::pallet_prelude::*;
	use webb_light_client_primitives::{
		traits::{LightClientHandler, ProofVerifier},
		types::LightProposalInput,
	};

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	/// The module configuration trait.
	pub trait Config: frame_system::Config + pallet_bridge_registry::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Light client interface for the pallet
		type LightClient: LightClientHandler;

		/// Storage proof verifier for the pallt
		type ProofVerifier: ProofVerifier;

		/// Proposer handler trait
		type ProposalHandler: ProposalHandlerTrait<
			MaxProposalLength = <Self as pallet::Config>::MaxProposalLength,
		>;

		/// Max length of submitted proof
		#[pallet::constant]
		type MaxProofSize: Get<u32> + TypeInfo + Clone + Debug + PartialEq + Eq;

		/// Max length of a proposal
		#[pallet::constant]
		type MaxProposalLength: Get<u32>
			+ Debug
			+ Clone
			+ Eq
			+ PartialEq
			+ PartialOrd
			+ Ord
			+ TypeInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		ProposalSubmitted { proposal: LightProposalInput },
	}

	#[pallet::storage]
	#[pallet::getter(fn resource_id_to_nonce)]
	/// Mapping of resource to nonce
	pub type ResourceIdToNonce<T: Config> = StorageMap<_, Blake2_256, ResourceId, u32, ValueQuery>;

	#[pallet::error]
	pub enum Error<T> {
		/// Cannot fetch bridge details
		CannotFetchBridgeDetails,
		/// Cannot fetch bridge metadata
		CannotFetchBridgeMetadata,
		/// Proof verification failed
		ProofVerificationFailed,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().reads_writes(1,1))]
		pub fn submit_proposal(
			origin: OriginFor<T>,
			proposal: LightProposalInput,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			// validate the block header against light client storage
			T::LightClient::verify_block_header_exists(
				proposal.block_header.clone(),
				proposal.resource_id.typed_chain_id(),
			)?;

			// validate the merkle proofs
			T::ProofVerifier::verify_storage_proof(
				proposal.clone().block_header,
				MERKLE_ROOT_KEY.to_vec(),
				proposal.clone().merkle_root_proof.to_vec(),
			)
			.map_err(|_| Error::<T>::ProofVerificationFailed)?;

			T::ProofVerifier::verify_storage_proof(
				proposal.clone().block_header,
				LEAF_INDEX_KEY.to_vec(),
				proposal.clone().leaf_index_proof.to_vec(),
			)
			.map_err(|_| Error::<T>::ProofVerificationFailed)?;

			// prepare the proposals to all linked bridges
			Self::submit_anchor_update_proposals(proposal.clone())?;

			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Submits anchor update proposals for all registered bridges from bridge registry
	///
	/// This function takes a `TypedChainId` representing the chain ID and a `LightProposalInput`
	/// type `proposal` containing the proposal details.
	///
	/// The function iterates through all registered bridges on the chain and creates an anchor
	/// update proposal for each one.
	///
	/// # Arguments
	///
	/// * `typed_chain_id` - The typed chain ID.
	/// * `proposal` - A `LightProposalInput` containing the details of the proposal.
	///
	/// # Errors
	///
	/// Returns a `DispatchError` if there was an issue during proposal submission.
	pub fn submit_anchor_update_proposals(
		proposal: LightProposalInput,
	) -> Result<(), DispatchError> {
		let src_resource_id = proposal.resource_id;

		// fetch bridge index for source_resource_id
		let bridge_index = pallet_bridge_registry::ResourceToBridgeIndex::<T>::get(src_resource_id)
			.ok_or(Error::<T>::CannotFetchBridgeDetails)?;

		// get all connected resource_ids
		let bridge_metadata = pallet_bridge_registry::Bridges::<T>::get(bridge_index)
			.ok_or(Error::<T>::CannotFetchBridgeMetadata)?;

		for resource_id in bridge_metadata.resource_ids.iter() {
			// create AUP for resource_id
			Self::create_and_submit_anchor_update_proposal(
				proposal.clone(),
				src_resource_id,
				*resource_id,
			)?;
		}

		// emit event
		Self::deposit_event(Event::ProposalSubmitted { proposal: proposal.clone() });

		Ok(())
	}

	/// Creates and submits an anchor update proposal.
	///
	/// This function takes a `LightProposalInput` type `proposal`, representing the proposal
	/// details, `src_resource_id` representing the source resource ID, and `target_resource_id`
	/// representing the target resource ID.
	///
	/// # Arguments
	///
	/// * `proposal` - A `LightProposalInput` containing the details of the proposal.
	/// * `src_resource_id` - The resource ID of the source.
	/// * `target_resource_id` - The resource ID of the target.
	///
	/// # Errors
	///
	/// Returns a `DispatchError` if there was an issue during proposal submission.
	pub fn create_and_submit_anchor_update_proposal(
		proposal: LightProposalInput,
		src_resource_id: ResourceId,
		target_resource_id: ResourceId,
	) -> Result<(), DispatchError> {
		// get the nonce used for target resrouce id
		let nonce = ResourceIdToNonce::<T>::get(target_resource_id);
		// update the nonce
		ResourceIdToNonce::<T>::insert(target_resource_id, nonce.saturating_add(1u32));
		let function_signature_bytes = v_anchor_contract::UpdateEdgeCall::selector();

		// prep the proposal
		let proposal = AnchorUpdateProposal::new(
			ProposalHeader::new(
				target_resource_id,
				FunctionSignature::from(function_signature_bytes),
				Nonce(nonce),
			),
			proposal.merkle_root,
			src_resource_id,
		);

		let unsigned_anchor_update_proposal = Proposal::Unsigned {
			kind: ProposalKind::AnchorUpdate,
			data: proposal.into_bytes().to_vec().try_into().unwrap(),
		};

		// submit the proposal
		T::ProposalHandler::handle_unsigned_proposal(unsigned_anchor_update_proposal)?;

		Ok(())
	}
}
