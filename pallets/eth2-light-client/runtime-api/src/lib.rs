// This file is part of Webb.

// Copyright (C) Webb Technologies, Inc.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Runtime API definition required by System RPC extensions.
//!
//! This API should be imported and implemented by the runtime,
//! of a node that wants to use the custom RPC extension
//! adding System access methods.

#![cfg_attr(not(feature = "std"), no_std)]

pub use eth_types::{H256, pallet::ClientMode, eth2::LightClientState};
pub use webb_proposals::TypedChainId;

sp_api::decl_runtime_apis! {
	pub trait Eth2LightClientApi<AccountId: codec::Codec> {
        /// Gets finalized beacon block hash from Ethereum Light Client on Substrate
        fn get_finalized_beacon_block_hash(typed_chain_id: TypedChainId) -> H256;

        /// Gets finalized beacon block slot from Ethereum Light Client on Substrate
        fn get_finalized_beacon_block_slot(typed_chain_id: TypedChainId) -> u64;

        /// Gets the current client mode of the Ethereum Light Client on Substrate
        fn get_client_mode(typed_chain_id: TypedChainId) -> ClientMode;

        /// Gets the Light Client State of the Ethereum Light Client on Substrate
        fn get_light_client_state(typed_chain_id: TypedChainId) -> LightClientState;

        fn get_last_block_number(typed_chain_id: TypedChainId) -> u64;

        fn get_unfinalized_tail_block_number(typed_chain_id: TypedChainId) -> Option<u64>;
	}
}