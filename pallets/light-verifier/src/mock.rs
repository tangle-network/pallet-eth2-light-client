use super::*;
use crate as pallet_light_verifier;

use codec::{Decode, Encode};
use consensus::network_config::{Network, NetworkConfig};

use dkg_runtime_primitives::TypedChainId;
use frame_support::{parameter_types, sp_io, PalletId};
use frame_system as system;

use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
	AccountId32, BuildStorage, MultiSignature,
};
use sp_std::convert::{TryFrom, TryInto};

pub type Signature = MultiSignature;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		Eth2Client: pallet_eth2_light_client,
		LightProposals: pallet_light_verifier
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

impl system::Config for Test {
	type AccountData = pallet_balances::AccountData<u128>;
	type AccountId = AccountId;
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockHashCount = BlockHashCount;
	type BlockLength = ();
	type Nonce = u64;
	type Block = frame_system::mocking::MockBlock<Test>;
	type BlockWeights = ();
	type RuntimeCall = RuntimeCall;
	type DbWeight = ();
	type RuntimeEvent = RuntimeEvent;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type Lookup = IdentityLookup<Self::AccountId>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type OnKilledAccount = ();
	type OnNewAccount = ();
	type OnSetCode = ();
	type RuntimeOrigin = RuntimeOrigin;
	type PalletInfo = PalletInfo;
	type SS58Prefix = SS58Prefix;
	type SystemWeightInfo = ();
	type Version = ();
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Config for Test {
	type AccountStore = System;
	type Balance = u128;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type FreezeIdentifier = ();
	type MaxHolds = ();
	type MaxFreezes = ();
	type WeightInfo = ();
	type RuntimeHoldReason = RuntimeHoldReason;
}

parameter_types! {
	#[derive(serde::Serialize, serde::Deserialize)]
	pub const MaxAdditionalFields: u32 = 5;
	#[derive(serde::Serialize, serde::Deserialize)]
	pub const MaxResources: u32 = 32;
	pub const StoragePricePerByte: u128 = 1;
	pub const Eth2ClientPalletId: PalletId = PalletId(*b"py/eth2c");
}

impl pallet_eth2_light_client::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type StoragePricePerByte = StoragePricePerByte;
	type PalletId = Eth2ClientPalletId;
	type Currency = Balances;
}

parameter_types! {
	#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, scale_info::TypeInfo, Ord, PartialOrd)]
	pub const MaxProposalLength : u32 = 10_000;
}

impl pallet_light_verifier::Config for Test {
	type RuntimeEvent = RuntimeEvent;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut storage = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let _ = pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(AccountId32::new([1u8; 32]), 10u128.pow(18)),
			(AccountId32::new([2u8; 32]), 20u128.pow(18)),
			(AccountId32::new([3u8; 32]), 30u128.pow(18)),
		],
	}
	.assimilate_storage(&mut storage);
	let _ = pallet_eth2_light_client::GenesisConfig::<Test> {
		phantom: Default::default(),
		networks: vec![
			(
				// Mainnet
				TypedChainId::Evm(1),
				NetworkConfig::new(&Network::Mainnet),
			),
			(
				// Goerli
				TypedChainId::Evm(5),
				NetworkConfig::new(&Network::Goerli),
			),
		],
	}
	.assimilate_storage(&mut storage);

	storage.into()
}
