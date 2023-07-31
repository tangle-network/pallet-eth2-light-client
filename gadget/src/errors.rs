#[derive(Debug)]
pub struct FailedToLoadKeys;

impl std::fmt::Display for FailedToLoadKeys {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Failed to load keys")
	}
}

impl std::error::Error for FailedToLoadKeys {}

#[derive(Debug)]
pub struct InvalidDirectory;

impl std::fmt::Display for InvalidDirectory {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Invalid directory")
	}
}

impl std::error::Error for InvalidDirectory {}
