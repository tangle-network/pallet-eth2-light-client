use sp_core::Encode;
use webb::substrate::subxt::dynamic::Value;

pub trait AsValue: Encode {
	fn as_value(&self) -> Value;
}

impl<T: Encode> AsValue for T {
	fn as_value(&self) -> Value {
		Value::from_bytes(self.encode())
	}
}
