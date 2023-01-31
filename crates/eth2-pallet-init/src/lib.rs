pub mod config;
pub mod config_for_tests;
pub mod eth_client_pallet_trait;
pub mod eth_network;
pub mod init_pallet;
pub mod misc;
pub mod substrate_network;
pub mod substrate_pallet_client;

#[derive(Debug, Clone)]
pub struct Error {
    pub inner: String
}

impl<T: ToString> From<T> for Error {
    fn from(value: T) -> Self {
        Error { inner: value.to_string() }
    }
}