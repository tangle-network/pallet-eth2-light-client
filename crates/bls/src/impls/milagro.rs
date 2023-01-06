use crate::{
	generic_aggregate_public_key::TAggregatePublicKey,
	generic_aggregate_signature::TAggregateSignature,
	generic_public_key::{GenericPublicKey, TPublicKey, PUBLIC_KEY_BYTES_LEN},
	generic_secret_key::{TSecretKey, SECRET_KEY_BYTES_LEN},
	generic_signature::{TSignature, SIGNATURE_BYTES_LEN},
	Error, Hash256, ZeroizeHash,
};
use alloc::vec::Vec;
use core::iter::ExactSizeIterator;
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};
use crate::OkOr;

/// Provides the externally-facing, core BLS types.
pub mod types {
	pub use signature_bls::{MultiPublicKeyVt, AggregateSignatureVt, PublicKeyVt, SecretKey, SignatureVt};
	pub use super::{
		verify_signature_sets, SignatureSet,
	};
}

pub type SignatureSet<'a> = crate::generic_signature_set::GenericSignatureSet<
	'a,
	signature_bls::PublicKeyVt,
	signature_bls::MultiPublicKeyVt,
	signature_bls::SignatureVt,
	signature_bls::AggregateSignature,
>;

pub fn verify_signature_sets<'a>(
	signature_sets: impl ExactSizeIterator<Item = &'a SignatureSet<'a>>,
	seed: [u8; 32],
) -> bool {
	if signature_sets.len() == 0 {
		return false
	}

	signature_sets
		.map(|signature_set| {
			let mut aggregate = signature_bls::MultiPublicKeyVt::from(
				&[signature_set.signing_keys.first().ok_or(())?.point().clone()] as &[signature_bls::PublicKeyVt],
			);

			for signing_key in signature_set.signing_keys.iter().skip(1) {
				aggregate.add(signing_key.point())
			}

			if signature_set.signature.point().is_none() {
				return Err(())
			}

			Ok((signature_set.signature.as_ref(), aggregate, signature_set.message))
		})
		.collect::<Result<Vec<_>, ()>>()
		.map(|aggregates| {
			let mut rng: ChaCha20Rng = ChaCha20Rng::from_seed(seed);
			let data = aggregates.iter().map(|(signature, aggregate, message)| {
				(
					signature.point().clone(),
					message.as_bytes(),
				)
			}).collect::<Vec<_>>();
			signature_bls::AggregateSignatureVt::verify(&data).into()
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
		let public_key = signature_bls::PublicKeyVt::from_bytes(self.to_bytes()).unwrap();
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
		signature_bls::SignatureVt::from_bytes(&slice).ok_or(Error::BlsError(signature_bls::Error::InvalidShare))
	}

	fn verify(&self, pubkey: &signature_bls::PublicKeyVt, msg: Hash256) -> bool {
		signature_bls::SignatureVt::verify(self, pubkey.clone(), msg.as_bytes()).into()
	}
}

impl TAggregateSignature<signature_bls::PublicKeyVt, signature_bls::MultiPublicKeyVt, signature_bls::SignatureVt>
	for signature_bls::AggregateSignatureVt
{
	fn infinity() -> Self {
		signature_bls::AggregateSignatureVt::from_bytes(&[0u8; SIGNATURE_BYTES_LEN]).unwrap()
	}

	fn add_assign(&mut self, other: &signature_bls::SignatureVt) {
		self += other
	}

	fn add_assign_aggregate(&mut self, other: &Self) {
		self += other
	}

	fn serialize(&self) -> [u8; SIGNATURE_BYTES_LEN] {
		crate::fit_to_array(self.to_bytes()).unwrap()
	}

	fn deserialize(bytes: &[u8]) -> Result<Self, Error> {
		let slice = crate::fit_to_array(bytes)?;
		signature_bls::AggregateSignatureVt::from_bytes(&slice).ok_or(Error::BlsError(signature_bls::Error::InvalidShare))
	}

	fn fast_aggregate_verify(
		&self,
		msg: Hash256,
		pubkeys: &[&GenericPublicKey<signature_bls::PublicKeyVt>],
	) -> bool {
		let data = pubkeys.iter().map(|pkey| (pkey.point().clone(), msg.as_bytes())).collect::<Vec<_>>();
		signature_bls::AggregateSignatureVt::verify(self, &data).into()
	}

	fn aggregate_verify(
		&self,
		msgs: &[Hash256],
		pubkeys: &[&GenericPublicKey<signature_bls::PublicKeyVt>],
	) -> bool {
		let data = pubkeys.iter().zip(msgs.iter())
		.map(|(pkey, msg)| (pkey.point().clone(), msg.as_bytes()))
		.collect::<Vec<_>>();
		signature_bls::AggregateSignatureVt::verify(self, &data).into()
	}
}

impl TSecretKey<signature_bls::SignatureVt, signature_bls::PublicKeyVt> for signature_bls::SecretKey {
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