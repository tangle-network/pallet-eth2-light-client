use serde::{Deserialize, Serialize};
use std::{
	error::Error,
	fmt,
	fmt::{Display, Formatter},
	str::FromStr,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubstrateNetwork {
	Mainnet,
	Testnet,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IncorrectSubstrateNetwork;

impl Display for IncorrectSubstrateNetwork {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Unknown Substrate network id. Possible networks: 'Mainnet', 'Testnet'")
	}
}

impl Error for IncorrectSubstrateNetwork {}

impl Display for SubstrateNetwork {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl SubstrateNetwork {
	pub fn as_str(&self) -> &str {
		match self {
			SubstrateNetwork::Mainnet => "mainnet",
			SubstrateNetwork::Testnet => "testnet",
		}
	}
}

impl FromStr for SubstrateNetwork {
	type Err = IncorrectSubstrateNetwork;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_lowercase().as_str() {
			"mainnet" => Ok(SubstrateNetwork::Mainnet),
			"testnet" => Ok(SubstrateNetwork::Testnet),
			_ => Err(IncorrectSubstrateNetwork),
		}
	}
}
