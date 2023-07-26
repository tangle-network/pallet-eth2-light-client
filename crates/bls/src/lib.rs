//! This library provides a wrapper around several BLS implementations to provide
//! Lighthouse-specific functionality.
//!
//! This crate should not perform direct cryptographic operations, instead it should do these via
//! external libraries. However, seeing as it is an interface to a real cryptographic library, it
//! may contain logic that affects the outcomes of cryptographic operations.
//!
//! A source of complexity in this crate is that *multiple* BLS implementations (a.k.a. "backends")
//! are supported via compile-time flags. There are three backends supported via features:
//!
//! - `milagro`: the classic pure-Rust `milagro_bls` crate.
//!
//! This crate uses traits to reduce code-duplication between the two implementations. For example,
//! the `GenericPublicKey` struct exported from this crate is generic across the `TPublicKey` trait
//! (i.e., `PublicKey<TPublicKey>`). `TPublicKey` is implemented by all three backends (see the
//! `impls.rs` module). When compiling with the `milagro` feature, we export
//! `type PublicKey = GenericPublicKey<milagro::PublicKey>`.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[macro_use]
mod macros;
mod generic_aggregate_public_key;
mod generic_aggregate_signature;
mod generic_keypair;
mod generic_public_key;
mod generic_public_key_bytes;
mod generic_secret_key;
mod generic_signature;
mod generic_signature_bytes;
mod generic_signature_set;
mod get_withdrawal_credentials;
mod zeroize_hash;

pub mod impls;

pub use generic_public_key::{INFINITY_PUBLIC_KEY, PUBLIC_KEY_BYTES_LEN};
pub use generic_secret_key::SECRET_KEY_BYTES_LEN;
pub use generic_signature::{INFINITY_SIGNATURE, SIGNATURE_BYTES_LEN};
pub use get_withdrawal_credentials::get_withdrawal_credentials;
pub use zeroize_hash::ZeroizeHash;

use milagro_bls::AmclError;

pub type Hash256 = ethereum_types::H256;

#[derive(Clone, Debug, PartialEq)]
pub enum Error {
	/// An error was raised from the Milagro BLS library.
	MilagroError(AmclError),
	/// The provided bytes were an incorrect length.
	InvalidByteLength { got: usize, expected: usize },
	/// The provided secret key bytes were an incorrect length.
	InvalidSecretKeyLength { got: usize, expected: usize },
	/// The public key represents the point at infinity, which is invalid.
	InvalidInfinityPublicKey,
	/// The secret key is all zero bytes, which is invalid.
	InvalidZeroSecretKey,
}

#[cfg(feature = "std")]
impl std::fmt::Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Error::MilagroError(_) => write!(f, "MilagroError"),
			Error::InvalidByteLength { got, expected } =>
				write!(f, "InvalidByteLength {{ got: {got}, expected: {expected} }}"),
			Error::InvalidSecretKeyLength { got, expected } =>
				write!(f, "InvalidSecretKeyLength {{ got: {got}, expected: {expected} }}"),
			Error::InvalidInfinityPublicKey => write!(f, "InvalidInfinityPublicKey"),
			Error::InvalidZeroSecretKey => write!(f, "InvalidZeroSecretKey"),
		}
	}
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

impl From<AmclError> for Error {
	fn from(e: AmclError) -> Error {
		Error::MilagroError(e)
	}
}

/// Generic implementations which are only generally useful for docs.
pub mod generics {
	pub use crate::{
		generic_aggregate_public_key::GenericAggregatePublicKey,
		generic_aggregate_signature::GenericAggregateSignature, generic_keypair::GenericKeypair,
		generic_public_key::GenericPublicKey, generic_public_key_bytes::GenericPublicKeyBytes,
		generic_secret_key::GenericSecretKey, generic_signature::GenericSignature,
		generic_signature_bytes::GenericSignatureBytes,
	};
}

/// Defines all the fundamental BLS points which should be exported by this crate by making
/// concrete the generic type parameters using the points from some external BLS library (e.g.,
/// Milagro, BLST).
macro_rules! define_mod {
	($name: ident, $mod: path) => {
		pub mod $name {
			use $mod as bls_variant;

			use crate::generics::*;

			pub use bls_variant::{verify_signature_sets, SignatureSet};

			pub type PublicKey = GenericPublicKey<bls_variant::PublicKey>;
			pub type PublicKeyBytes = GenericPublicKeyBytes<bls_variant::PublicKey>;
			pub type AggregatePublicKey =
				GenericAggregatePublicKey<bls_variant::PublicKey, bls_variant::AggregatePublicKey>;
			pub type Signature = GenericSignature<bls_variant::PublicKey, bls_variant::Signature>;
			pub type AggregateSignature = GenericAggregateSignature<
				bls_variant::PublicKey,
				bls_variant::AggregatePublicKey,
				bls_variant::Signature,
				bls_variant::AggregateSignature,
			>;
			pub type SignatureBytes =
				GenericSignatureBytes<bls_variant::PublicKey, bls_variant::Signature>;
			pub type SecretKey = GenericSecretKey<
				bls_variant::Signature,
				bls_variant::PublicKey,
				bls_variant::SecretKey,
			>;
			pub type Keypair = GenericKeypair<
				bls_variant::PublicKey,
				bls_variant::SecretKey,
				bls_variant::Signature,
			>;
		}
	};
}

define_mod!(milagro_implementations, crate::impls::milagro::types);
pub use milagro_implementations::*;
