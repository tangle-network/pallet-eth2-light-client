use crate::{
	generic_aggregate_public_key::TAggregatePublicKey,
	generic_aggregate_signature::TAggregateSignature,
	generic_public_key::{GenericPublicKey, TPublicKey, PUBLIC_KEY_BYTES_LEN},
	generic_secret_key::{TSecretKey, SECRET_KEY_BYTES_LEN},
	generic_signature::{TSignature, SIGNATURE_BYTES_LEN},
	Error, Hash256, OkOr, ZeroizeHash,
};
use alloc::vec::Vec;
use core::iter::ExactSizeIterator;
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};

/// Provides the externally-facing, core BLS types.
pub mod types {
	pub use super::{verify_signature_sets, SignatureSet};
	pub use signature_bls::{
		AggregateSignatureVt, MultiPublicKeyVt, PublicKeyVt, SecretKey, SignatureVt,
	};
}

pub type SignatureSet<'a> = crate::generic_signature_set::GenericSignatureSet<
	'a,
	signature_bls::PublicKeyVt,
	signature_bls::MultiPublicKeyVt,
	signature_bls::SignatureVt,
	signature_bls::AggregateSignatureVt,
>;

pub fn verify_signature_sets<'a>(
	signature_sets: impl ExactSizeIterator<Item = &'a SignatureSet<'a>>,
	_seed: [u8; 32],
) -> bool {
	if signature_sets.len() == 0 {
		return false
	}

	signature_sets
		.map(|signature_set| {
			// this signature set signed a single message
			let signing_keys =
				signature_set.signing_keys.iter().map(|r| r.point().clone()).collect::<Vec<_>>();
			let verify_input = signing_keys
				.into_iter()
				.map(|r| (r, signature_set.message.as_bytes()))
				.collect::<Vec<_>>();

			if signature_set.signature.point().is_none() {
				return Err(())
			}

			Ok((signature_set.signature.point().unwrap(), verify_input))
		})
		.collect::<Result<Vec<_>, ()>>()
		.map(|aggregates| {
			aggregates
				.iter()
				.all(|(agg_sig, verify_input)| agg_sig.verify(&verify_input).into())
		})
		.unwrap_or(false)
}

impl TPublicKey for signature_bls::PublicKeyVt {
	fn serialize(&self) -> [u8; PUBLIC_KEY_BYTES_LEN] {
		crate::fit_to_array(self.to_bytes()).unwrap()
	}

	fn deserialize(bytes: [u8; PUBLIC_KEY_BYTES_LEN]) -> Result<Self, Error> {
		Self::from_bytes(&bytes).ok_or(Error::BlsError(signature_bls::Error::InvalidShare))
	}
}

impl TAggregatePublicKey<signature_bls::PublicKeyVt> for signature_bls::MultiPublicKeyVt {
	fn to_public_key(&self) -> GenericPublicKey<signature_bls::PublicKeyVt> {
		let public_key = signature_bls::PublicKeyVt::from_bytes(&self.to_bytes()).unwrap();
		GenericPublicKey::from_point(public_key)
	}

	fn aggregate(pubkeys: &[GenericPublicKey<signature_bls::PublicKeyVt>]) -> Result<Self, Error> {
		let pubkey_refs = pubkeys.iter().map(|pk| pk.point().clone()).collect::<Vec<_>>();
		Ok(pubkey_refs.as_slice().into())
	}
}

impl TSignature<signature_bls::PublicKeyVt> for signature_bls::SignatureVt {
	fn serialize(&self) -> [u8; SIGNATURE_BYTES_LEN] {
		crate::fit_to_array(self.to_bytes()).unwrap()
	}

	fn deserialize(bytes: &[u8]) -> Result<Self, Error> {
		let slice = crate::fit_to_array(bytes)?;
		signature_bls::SignatureVt::from_bytes(&slice)
			.ok_or(Error::BlsError(signature_bls::Error::InvalidShare))
	}

	fn verify(&self, pubkey: &signature_bls::PublicKeyVt, msg: Hash256) -> bool {
		signature_bls::SignatureVt::verify(self, pubkey.clone(), msg.as_bytes()).into()
	}
}

impl
	TAggregateSignature<
		signature_bls::PublicKeyVt,
		signature_bls::MultiPublicKeyVt,
		signature_bls::SignatureVt,
	> for signature_bls::AggregateSignatureVt
{
	fn infinity() -> Self {
		signature_bls::AggregateSignatureVt::default()
	}

	fn add_assign(&mut self, other: &signature_bls::SignatureVt) {
		// convert each to bytes
		let bytes_other = other.to_bytes();
		let bytes_self = self.to_bytes();
		// convert the byte reprs into an individual signature vt and add to array
		let concat = &[
			signature_bls::SignatureVt::from_bytes(&bytes_other).unwrap(),
			signature_bls::SignatureVt::from_bytes(&bytes_self).unwrap(),
		];

		let new =
			signature_bls::AggregateSignatureVt::from(concat as &[signature_bls::SignatureVt]);
		*self = new
	}

	fn add_assign_aggregate(&mut self, other: &Self) {
		// convert each to bytes
		let bytes_other = other.to_bytes();
		let bytes_self = self.to_bytes();
		// convert the byte reprs into an individual signature vt and add to array
		let concat = &[
			signature_bls::SignatureVt::from_bytes(&bytes_other).unwrap(),
			signature_bls::SignatureVt::from_bytes(&bytes_self).unwrap(),
		];

		let new =
			signature_bls::AggregateSignatureVt::from(concat as &[signature_bls::SignatureVt]);
		*self = new
	}

	fn serialize(&self) -> [u8; SIGNATURE_BYTES_LEN] {
		crate::fit_to_array(self.to_bytes()).unwrap()
	}

	fn deserialize(bytes: &[u8]) -> Result<Self, Error> {
		let slice = crate::fit_to_array(bytes)?;
		signature_bls::AggregateSignatureVt::from_bytes(&slice)
			.ok_or(Error::BlsError(signature_bls::Error::InvalidShare))
	}

	fn fast_aggregate_verify(
		&self,
		msg: Hash256,
		pubkeys: &[&GenericPublicKey<signature_bls::PublicKeyVt>],
	) -> bool {
		let data = pubkeys
			.iter()
			.map(|pkey| (pkey.point().clone(), msg.as_bytes()))
			.collect::<Vec<_>>();
		signature_bls::AggregateSignatureVt::verify(self, &data).into()
	}

	fn aggregate_verify(
		&self,
		msgs: &[Hash256],
		pubkeys: &[&GenericPublicKey<signature_bls::PublicKeyVt>],
	) -> bool {
		let data = pubkeys
			.iter()
			.zip(msgs.iter())
			.map(|(pkey, msg)| (pkey.point().clone(), msg.as_bytes()))
			.collect::<Vec<_>>();
		signature_bls::AggregateSignatureVt::verify(self, &data).into()
	}
}

impl TSecretKey<signature_bls::SignatureVt, signature_bls::PublicKeyVt>
	for signature_bls::SecretKey
{
	fn random() -> Self {
		Self::random(&mut ChaCha20Rng::from_seed([1u8; 32])).unwrap()
	}

	fn public_key(&self) -> signature_bls::PublicKeyVt {
		self.into()
	}

	fn sign(&self, msg: Hash256) -> signature_bls::SignatureVt {
		signature_bls::SignatureVt::new(self, msg).unwrap()
	}

	fn serialize(&self) -> ZeroizeHash {
		crate::fit_to_array::<SECRET_KEY_BYTES_LEN>(self.to_bytes()).unwrap().into()
	}

	fn deserialize(bytes: &[u8]) -> Result<Self, Error> {
		let slice = crate::fit_to_array(bytes)?;
		Self::from_bytes(&slice).ok_or(signature_bls::Error::InvalidSecret.into())
	}
}
