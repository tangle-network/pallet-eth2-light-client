use async_trait::async_trait;

use eth_types::{
	eth2::{ExtendedBeaconBlockHeader, LightClientState, LightClientUpdate, SyncCommittee},
	BlockHeader, H256,
};
use sp_core::crypto::AccountId32;
use webb::substrate::{
	scale::{Decode, Encode},
	subxt::{
		self,
		storage::address::Yes,
		metadata::DecodeWithMetadata,
		dynamic::{DecodedValueThunk, Value},
		ext::sp_core::sr25519::Pair,
		tx::{PairSigner, TxPayload},
		OnlineClient, PolkadotConfig, storage::StorageAddress,
	},
};
use webb_proposals::TypedChainId;
use webb_relayer_utils::Error;
use subxt::ext::sp_core::Pair as PairT;

use crate::{
	eth_client_pallet_trait::EthClientPalletTrait,
};

pub async fn setup_api() -> Result<OnlineClient<PolkadotConfig>, Error> {
	let api: OnlineClient<PolkadotConfig> = OnlineClient::<PolkadotConfig>::new()
		.await
		.map_err(|_| Error::Generic("Failed to setup online client"))?;
	Ok(api)
}

pub struct EthClientPallet {
	api: OnlineClient<PolkadotConfig>,
	signer: PairSigner<PolkadotConfig, Pair>,
	chain: tangle::runtime_types::webb_proposals::header::TypedChainId,
	max_submitted_blocks_by_account: Option<u32>
}

impl EthClientPallet {
	pub fn new(api: OnlineClient<PolkadotConfig>) -> Self {
		Self::new_with_pair(api, sp_keyring::AccountKeyring::Alice.pair())
	}

	pub fn new_with_pair(api: OnlineClient<PolkadotConfig>, pair: Pair) -> Self {
		let signer = PairSigner::new(pair);
		Self { api, signer, chain: tangle::runtime_types::webb_proposals::header::TypedChainId::Evm(5), max_submitted_blocks_by_account: None }
	}

	pub fn new_with_suri_key<T: AsRef<str>>(api: OnlineClient<PolkadotConfig>, suri_key: T) -> Result<Self, crate::Error> {
		let pair = get_sr25519_keys_from_suri(suri_key)?;
		Ok(Self::new_with_pair(api, pair))
	}

	pub fn get_signer_account_id(&self) -> AccountId32 {
		(*AsRef::<[u8; 32]>::as_ref(&self.signer.account_id())).into()
	}

	pub async fn init(
		&mut self,
		typed_chain_id: TypedChainId,
		finalized_execution_header: BlockHeader,
		finalized_beacon_header: ExtendedBeaconBlockHeader,
		current_sync_committee: SyncCommittee,
		next_sync_committee: SyncCommittee,
		validate_updates: Option<bool>,
		verify_bls_signatures: Option<bool>,
		hashes_gc_threshold: Option<u64>,
		max_submitted_blocks_by_account: Option<u32>,
		trusted_signer: Option<AccountId32>,
	) -> Result<(), Error> {

		let max_submitted_blocks_by_account = max_submitted_blocks_by_account.unwrap_or(10);
		self.max_submitted_blocks_by_account = Some(max_submitted_blocks_by_account);

		let trusted_signer = if let Some(trusted_signer) = trusted_signer {
			let bytes: [u8;32] = *trusted_signer.as_ref();
			Some(subxt::ext::sp_runtime::AccountId32::from(bytes))
		} else {
			None
		};

		let init_input: tangle::runtime_types::eth_types::pallet::InitInput<subxt::ext::sp_runtime::AccountId32> = tangle::runtime_types::eth_types::pallet::InitInput {
			finalized_execution_header: Decode::decode(&mut finalized_execution_header.encode().as_slice())?,
			finalized_beacon_header: Decode::decode(&mut finalized_beacon_header.encode().as_slice())?,
			current_sync_committee: Decode::decode(&mut current_sync_committee.encode().as_slice())?,
			next_sync_committee: Decode::decode(&mut next_sync_committee.encode().as_slice())?,
			validate_updates: validate_updates.unwrap_or(true),
			verify_bls_signatures: verify_bls_signatures.unwrap_or(true),
			hashes_gc_threshold: hashes_gc_threshold.unwrap_or(100),
			max_submitted_blocks_by_account: max_submitted_blocks_by_account,
			trusted_signer,
		};

		let tx = tangle::tx().eth2_client().init(typed_chain_id.chain_id(), init_input);

		// submit the transaction with default params:
		let _hash = self.submit(&tx).await.map_err(|err|Error::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("{err:?}"))))?;

		Ok(())
	}

	pub async fn get_last_eth2_slot_on_tangle(
		&self,
		typed_chain_id: TypedChainId,
	) -> Result<u64, Error> {
		//let addr = tangle::storage().eth2_client().finalized_beacon_header(0);
		let storage_address = subxt::dynamic::storage(
			"Eth2Client",
			"FinalizedHeaderUpdate",
			vec![Value::from_bytes(&typed_chain_id.chain_id().to_be_bytes())],
		);
		let _finalized_header_update: DecodedValueThunk =
			self.api.storage().fetch_or_default(&storage_address, None).await.map_err(|_| {
				Error::Generic("Failed to get finalized header update storage value")
			})?;

		Ok(0)
	}

	async fn get_value_or_default<'a, Address: StorageAddress>(&self, key_addr: &Address) -> Result<<Address::Target as DecodeWithMetadata>::Target, crate::Error> 
		where
		Address: StorageAddress<IsFetchable = Yes, IsDefaultable = Yes> + 'a {
		let value = self.api.storage().fetch_or_default(key_addr, None)
			.await
			.map_err(|err| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get api storage value: {err:?}"))))?;
	
		Ok(value)
	}

	async fn get_value<'a, Address: StorageAddress>(&self, key_addr: &Address) -> Result<Option<<Address::Target as DecodeWithMetadata>::Target>, crate::Error> 
		where
		Address: StorageAddress<IsFetchable = Yes> + 'a {
		let value = self.api.storage().fetch(key_addr, None)
			.await
			.map_err(|err| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get api storage value: {err:?}"))))?;

		Ok(value)
	}

	async fn submit<Call: TxPayload>(&self, call: &Call) -> Result<H256, crate::Error> {
		let hash = self
			.api
			.tx()
			.sign_and_submit_default(call, &self.signer)
			.await
			.map_err(|err| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get hash storage value: {err:?}"))))?;
		
		Ok(hash.0.into())
	}
}

#[async_trait]
impl EthClientPalletTrait for EthClientPallet {
	async fn get_last_submitted_slot(&self) -> u64 {
		self.get_finalized_beacon_block_slot()
			.await
			.expect("Unable to obtain finalized beacon slot")
	}

	async fn is_known_block(
		&self,
		execution_block_hash: &eth_types::H256,
	) -> Result<bool, crate::Error> {
		let decoded: tangle::runtime_types::eth_types::H256 = Decode::decode(&mut execution_block_hash.encode().as_slice()).unwrap();
		let addr = tangle::storage().eth2_client().unfinalized_headers(&self.chain, decoded);
		self.get_value(&addr)
		.await.map(|r| r.is_some())
	}

	async fn send_light_client_update(
		&mut self,
		light_client_update: LightClientUpdate,
	) -> Result<(), crate::Error> {
		let decoded_lcu = Decode::decode(&mut light_client_update.encode().as_slice()).unwrap();
		let decoded_tcid = Decode::decode(&mut self.chain.encode().as_slice()).unwrap();
		let call = tangle::tx().eth2_client().submit_beacon_chain_light_client_update(decoded_tcid, decoded_lcu);
		self.submit(&call).await.map(|_|())
	}

	async fn get_finalized_beacon_block_hash(&self) -> Result<eth_types::H256, crate::Error> {
		let addr = tangle::storage().eth2_client().finalized_beacon_header(&self.chain);
		self.get_value(&addr).await
			.map(|r| eth_types::H256::from(r.unwrap().beacon_block_root.0))
	}

	async fn get_finalized_beacon_block_slot(&self) -> Result<u64, crate::Error> {
		let addr = tangle::storage().eth2_client().finalized_beacon_header(&self.chain);
		self.get_value(&addr).await
			.map(|r| r.unwrap().header.slot)
	}

	async fn send_headers(
		&mut self,
		_headers: &[BlockHeader],
		_end_slot: u64,
	) -> Result<(), crate::Error> {
		/*
		let mut txes = vec![];
		for header in headers {
			let decoded_header = Decode::decode(&mut header.encode().as_slice()).unwrap();
			let tx = tangle::tx().eth2_client().submit_execution_header(self.chain, decoded_header);
			let encoded_tx: Vec<u8> = tx.encode();
			let call = Call::Evm(encoded_tx);
			tangle::runtime_types::frame_system::pallet::Call::
			let tx = tangle::runtime_types::tangle_standalone_runtime::RuntimeCall::Eth2Client(call);
			txes.push(tx);
		}

		let batch_call = tangle::tx().utility().batch_all(txes);

		self.submit(&batch_call).await
			.map(|_| ())*/
		Err(crate::Error::from("send_headers is not implemented"))
	}

	async fn get_min_deposit(
		&self,
	) -> Result<crate::eth_client_pallet_trait::Balance, crate::Error> {
		let addr = tangle::storage().eth2_client().min_submitter_balance(&self.chain);
		self.get_value_or_default(&addr).await
	}

	async fn register_submitter(&self) -> Result<(), crate::Error> {
		let decoded_tcid = Decode::decode(&mut self.chain.encode().as_slice()).unwrap();
		let tx = tangle::tx().eth2_client().register_submitter(decoded_tcid);
		self.submit(&tx).await
			.map(|_| ())
	}

	async fn is_submitter_registered(
		&self,
		account_id: Option<AccountId32>,
	) -> Result<bool, crate::Error> {
		if let Some(account_id) = account_id {
			let bytes: [u8; 32] = *account_id.as_ref();
			let addr = tangle::storage().eth2_client().submitters(&self.chain, subxt::ext::sp_core::crypto::AccountId32::from(bytes));
			self.get_value(&addr).await
				.map(|r| r.is_some())
		} else {
			// TODO: determine what to do if none specified
			// for now, don't error
			Ok(false)
		}
	}

	async fn get_light_client_state(
		&self,
	) -> Result<eth_types::eth2::LightClientState, crate::Error> {
		let addr0 = tangle::storage().eth2_client().finalized_beacon_header(&self.chain);
		let task0 = self.get_value(&addr0);

		let addr1 = tangle::storage().eth2_client().current_sync_committee(&self.chain);
		let task1 = self.get_value(&addr1);

		let addr2 = tangle::storage().eth2_client().next_sync_committee(&self.chain);
		let task2 = self.get_value(&addr2);

		let (finalized_beacon_header, current_sync_committee, next_sync_committee) =
			tokio::try_join!(task0, task1, task2)?;

		match (finalized_beacon_header, current_sync_committee, next_sync_committee) {
			(Some(finalized_beacon_header), Some(current_sync_committee), Some(next_sync_committee)) => {
				Ok(LightClientState {
					finalized_beacon_header: Decode::decode(&mut finalized_beacon_header.encode().as_slice()).unwrap(),
					current_sync_committee: Decode::decode(&mut current_sync_committee.encode().as_slice()).unwrap(),
					next_sync_committee: Decode::decode(&mut next_sync_committee.encode().as_slice()).unwrap(),
				})
			}

			_ => {
				Err(crate::Error::from("Unable to obtain all three values"))
			}
		}
	}

	async fn get_num_of_submitted_blocks_by_account(&self) -> Result<u32, crate::Error> {
		let account_id = self.signer.account_id();
		let addr = tangle::storage().eth2_client().submitters(&self.chain, account_id);
		if let Some(val) = self.get_value(&addr).await? {
			Ok(val)
		} else {
			Err(crate::Error::from("Failed to get value for get_num_of_submitted_blocks_by_account"))
		}
	}

	async fn get_max_submitted_blocks_by_account(&self) -> Result<u32, crate::Error> {
		let key_addr = tangle::storage().eth2_client().max_unfinalized_blocks_per_submitter(&self.chain);
		
		let value: u32 = self.api.storage().fetch_or_default(&key_addr, None)
			.await
			.map_err(|err| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get api storage value: {err:?}"))))?;

		Ok(value)
	}
}

fn get_sr25519_keys_from_suri<T: AsRef<str>>(suri: T) -> Result<Pair, crate::Error> {
	let value = suri.as_ref();
	if value.starts_with('$') {
		// env
		let var = value.strip_prefix('$').unwrap_or(value);
		log::trace!("Reading {} from env", var);
		let val = std::env::var(var).map_err(|e| {
			crate::Error::from(format!(
				"error while loading this env {var}: {e}",
			))
		})?;
		let maybe_pair =
			Pair::from_string(&val, None);
		match maybe_pair {
			Ok(pair) => Ok(pair),
			Err(e) => {
				Err(format!("{e:?}").into())
			}
		}
	} else if value.starts_with('>') {
		todo!("Implement command execution to extract the private key")
	} else {
		let maybe_pair =
			Pair::from_string(value, None);
		match maybe_pair {
			Ok(pair) => Ok(pair),
			Err(e) => {
				Err(format!("{e:?}").into())
			}
		}
	}
}

#[subxt::subxt(runtime_metadata_path = "../../metadata/eth_light_client_runtime.scale")]
pub mod tangle {}