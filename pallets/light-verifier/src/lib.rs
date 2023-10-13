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

use eth_types::BlockHeader;
use frame_support::{pallet_prelude::DispatchError, traits::Get};
pub use pallet::*;

use sp_std::{convert::TryInto, prelude::*};

use webb_light_client_primitives::traits::ProofVerifier;

/// Function sig for anchor update
pub const ANCHOR_UPDATE_FUNCTION_SIGNATURE: [u8; 4] = [0x26, 0x57, 0x88, 0x01];

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod verify;

use crate::verify::TrieProver;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	/// The module configuration trait.
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {
		/// Cannot verify storage proof
		CannotVerifyStorageProof,
		/// Cannot verify transaction proof
		CannotVerifyTransactionProof,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}
}

impl<T: Config> ProofVerifier for Pallet<T> {
	fn verify_storage_proof(
		header: BlockHeader,
		key: Vec<u8>,
		proof: Vec<Vec<u8>>,
	) -> Result<bool, DispatchError> {
		/*
		eth_getProof response :
		{
			"id": 1,
			"jsonrpc": "2.0",
			"result": {
				"accountProof": [
				"0xf90211a...0701bc80",
				"0xf90211a...0d832380",
				"0xf90211a...5fb20c80",
				"0xf90211a...0675b80",
				"0xf90151a0...ca08080"
				],
				"balance": "0x0",
				"codeHash": "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470",
				"nonce": "0x0",
				"storageHash": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
				"storageProof": [
				{
					"key": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
					"proof": [
					"0xf90211a...0701bc80",
					"0xf90211a...0d832380"
					],
					"value": "0x1"
				}
				]
			}
		}
		*/
		let storage_hash = header.calculate_hash();

		TrieProver::verify_trie_proof(storage_hash.into(), key, proof)
			.map_err(|_| Error::<T>::CannotVerifyStorageProof);

		Ok(true)
	}
}
