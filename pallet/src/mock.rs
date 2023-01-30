use super::*;
use crate as pallet_eth2_light_client;

use frame_support::{parameter_types, sp_io, traits::GenesisBuild};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
	AccountId32, MultiSignature,
};
use sp_std::convert::{TryFrom, TryInto};

pub type Signature = MultiSignature;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
		Eth2Client: pallet_eth2_light_client::{Pallet, Call, Storage, Event<T>},
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
	type BlockNumber = u64;
	type BlockWeights = ();
	type RuntimeCall = RuntimeCall;
	type DbWeight = ();
	type RuntimeEvent = RuntimeEvent;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type Header = Header;
	type Index = u64;
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
	type WeightInfo = ();
}

parameter_types! {
	pub const MaxAdditionalFields: u32 = 5;
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

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut storage = system::GenesisConfig::default().build_storage::<Test>().unwrap();
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
				[
					0x4b, 0x36, 0x3d, 0xb9, 0x4e, 0x28, 0x61, 0x20, 0xd7, 0x6e, 0xb9, 0x05, 0x34,
					0x0f, 0xdd, 0x4e, 0x54, 0xbf, 0xe9, 0xf0, 0x6b, 0xf3, 0x3f, 0xf6, 0xcf, 0x5a,
					0xd2, 0x7f, 0x51, 0x1b, 0xfe, 0x95,
				],
				[0x02, 0x00, 0x00, 0x00],
				18446744073709551615,
			),
			(
				// Goerli
				TypedChainId::Evm(5),
				[
					0x04, 0x3d, 0xb0, 0xd9, 0xa8, 0x38, 0x13, 0x55, 0x1e, 0xe2, 0xf3, 0x34, 0x50,
					0xd2, 0x37, 0x97, 0x75, 0x7d, 0x43, 0x09, 0x11, 0xa9, 0x32, 0x05, 0x30, 0xad,
					0x8a, 0x0e, 0xab, 0xc4, 0x3e, 0xfb,
				],
				[0x02, 0x00, 0x10, 0x20],
				112260,
			),
			(
				// Kiln
				TypedChainId::Evm(1337802),
				[
					0x99, 0xb0, 0x9f, 0xcd, 0x43, 0xe5, 0x90, 0x52, 0x36, 0xc3, 0x70, 0xf1, 0x84,
					0x05, 0x6b, 0xec, 0x6e, 0x66, 0x38, 0xcf, 0xc3, 0x1a, 0x32, 0x3b, 0x30, 0x4f,
					0xc4, 0xaa, 0x78, 0x9c, 0xb4, 0xad,
				],
				[0x70, 0x00, 0x00, 0x71],
				150,
			),
			(
				// Ropsten
				TypedChainId::Evm(3),
				[
					0x44, 0xf1, 0xe5, 0x62, 0x83, 0xca, 0x88, 0xb3, 0x5c, 0x78, 0x9f, 0x7f, 0x44,
					0x9e, 0x52, 0x33, 0x9b, 0xc1, 0xfe, 0xfe, 0x3a, 0x45, 0x91, 0x3a, 0x43, 0xa6,
					0xd1, 0x6e, 0xdc, 0xd3, 0x3c, 0xf1,
				],
				[0x80, 0x00, 0x00, 0x71],
				750,
			),
		],
	}
	.assimilate_storage(&mut storage);

	storage.into()
}
