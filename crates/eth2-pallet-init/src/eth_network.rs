use serde::{Deserialize, Serialize};
use std::{
	error::Error,
	fmt,
	fmt::{Display, Formatter},
	str::FromStr,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EthNetwork {
	Mainnet,
	Goerli,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IncorrectEthNetwork;

impl Display for IncorrectEthNetwork {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Unknown Ethereum network. Possible networks: 'Mainnet', 'Goerli'")
	}
}

impl Error for IncorrectEthNetwork {}

impl Display for EthNetwork {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl EthNetwork {
	pub fn as_str(&self) -> &str {
		match self {
			EthNetwork::Mainnet => "mainnet",
			EthNetwork::Goerli => "goerli",
		}
	}
}

impl FromStr for EthNetwork {
	type Err = IncorrectEthNetwork;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_lowercase().as_str() {
			"mainnet" => Ok(EthNetwork::Mainnet),
			"goerli" => Ok(EthNetwork::Goerli),
			_ => Err(IncorrectEthNetwork),
		}
	}
}
