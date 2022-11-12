use serde::Deserialize;
use std::{
	error::Error,
	fmt,
	fmt::{Display, Formatter},
	str::FromStr,
};

#[derive(Debug, Clone, Deserialize)]
pub enum ContractType {
	Pallet,
	Dao,
	File,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IncorrectContractType;

impl Display for IncorrectContractType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Unknown contract type. Possible contract types: 'Pallet', 'Dao', 'File'")
	}
}

impl Error for IncorrectContractType {}

impl Display for ContractType {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl ContractType {
	pub fn as_str(&self) -> &str {
		match self {
			ContractType::Pallet => "Pallet",
			ContractType::Dao => "Dao",
			ContractType::File => "File",
		}
	}
}

impl FromStr for ContractType {
	type Err = IncorrectContractType;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_lowercase().as_str() {
			"pallet" => Ok(ContractType::Pallet),
			"dao" => Ok(ContractType::Dao),
			"file" => Ok(ContractType::File),
			_ => Err(IncorrectContractType),
		}
	}
}
