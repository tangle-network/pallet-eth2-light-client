extern crate dotenv;

pub mod beacon_block_body_merkle_tree;
pub mod beacon_rpc_client;
pub mod config_for_tests;
pub mod errors;
pub mod eth1_rpc_client;
pub mod execution_block_proof;
pub mod hand_made_finality_light_client_update;
pub mod light_client_snapshot_with_proof;
pub mod utils;

use crate::errors::NoBlockForSlotError;
#[derive(Debug, Clone)]
pub struct Error {
	pub inner: String,
	pub is_no_block_for_slot_error: Option<NoBlockForSlotError>,
}

impl<T: ToString + 'static> From<T> for Error {
	fn from(value: T) -> Self {
		let value_any = &value as &dyn std::any::Any;
		let is_no_block_for_slot_error = value_any.downcast_ref::<NoBlockForSlotError>().cloned();
		Error { inner: value.to_string(), is_no_block_for_slot_error }
	}
}
