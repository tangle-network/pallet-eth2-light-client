pub mod config;
pub mod config_for_tests;
pub mod eth_client_pallet_trait;
pub mod eth_network;
pub mod init_pallet;
pub mod misc;
pub mod substrate_network;
pub mod substrate_pallet_client;

use eth_rpc_client::errors::NoBlockForSlotError;
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
