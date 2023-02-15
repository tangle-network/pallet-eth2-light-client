use async_trait::async_trait;

use eth_types::{
	eth2::{ExtendedBeaconBlockHeader, LightClientState, LightClientUpdate, SyncCommittee},
	pallet::ExecutionHeaderInfo,
	BlockHeader,
};
use sp_core::crypto::AccountId32;
use webb::substrate::{
	scale::{Decode, Encode},
	subxt::{
		self,
		dynamic::{DecodedValueThunk, Value},
		ext::sp_core::sr25519::Pair,
		tx::PairSigner,
		OnlineClient, PolkadotConfig,
	},
};
use webb_proposals::TypedChainId;
use webb_relayer_utils::Error;
use subxt::ext::sp_core::Pair as PairT;

use crate::{
	eth_client_pallet_trait::{Balance, EthClientPalletTrait},
	misc::AsValue,
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
	chain: TypedChainId,
}

impl EthClientPallet {
	pub fn new(api: OnlineClient<PolkadotConfig>) -> Self {
		Self::new_with_pair(api, sp_keyring::AccountKeyring::Alice.pair())
	}

	pub fn new_with_pair(api: OnlineClient<PolkadotConfig>, pair: Pair) -> Self {
		let signer = PairSigner::new(pair);
		Self { api, signer, chain: TypedChainId::Evm(5) }
	}

	pub fn new_with_suri_key<T: AsRef<str>>(api: OnlineClient<PolkadotConfig>, suri_key: T) -> Result<Self, crate::Error> {
		let pair = get_sr25519_keys_from_suri(suri_key)?;
		Ok(Self::new_with_pair(api, pair))
	}

	pub fn get_signer_account_id(&self) -> AccountId32 {
		(*AsRef::<[u8; 32]>::as_ref(&self.signer.account_id())).into()
	}

	pub async fn init(
		&self,
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
			max_submitted_blocks_by_account: max_submitted_blocks_by_account.unwrap_or(10),
			trusted_signer,
		};

		let tx = tangle::tx().eth2_client().init(typed_chain_id.chain_id(), init_input);

		// submit the transaction with default params:
		let _hash = self
			.api
			.tx()
			.sign_and_submit_default(&tx, &self.signer)
			.await
			.map_err(|err| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get hash storage value: {err:?}"))))?;

		Ok(())
	}

	pub async fn get_last_eth2_slot_on_tangle(
		&self,
		typed_chain_id: TypedChainId,
	) -> Result<u64, Error> {
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

	// Gets a value from subxt storage with multiple arguments
	async fn get_value_with_keys<T: Decode>(
		&self,
		entry_name: &str,
		keys: impl Into<Vec<Value>>,
	) -> Result<T, Error> {
		let storage_address = subxt::dynamic::storage("Eth2Client", entry_name, keys.into());

		let maybe_existant_value: DecodedValueThunk = self
			.api
			.storage()
			.fetch_or_default(&storage_address, None)
			.await
			.map_err(|_| Error::Generic("Failed to get api storage value"))?;

		let finalized_value: T = T::decode(&mut maybe_existant_value.encoded())?;

		Ok(finalized_value)
	}

	// Gets a value from subxt storage where the only input argument is the type chain id
	async fn get_value_with_simple_type_chain_argument<T: Decode>(
		&self,
		entry_name: &str,
	) -> Result<T, Error> {
		self.get_value_with_keys::<T>(entry_name, vec![self.get_type_chain_argument()])
			.await
	}

	fn get_type_chain_argument(&self) -> Value {
		self.chain.chain_id().as_value()
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
		let exists = self
			.get_value_with_keys::<Option<ExecutionHeaderInfo<AccountId32>>>(
				"UnfinalizedHeaders",
				vec![self.get_type_chain_argument(), execution_block_hash.as_value()],
			)
			.await
			.map(|r| r.is_some())?;

		Ok(exists)
	}

	async fn send_light_client_update(
		&mut self,
		light_client_update: LightClientUpdate,
	) -> Result<(), crate::Error> {
		let tx = subxt::dynamic::tx(
			"Eth2Client",
			// Name of the transaction in the pallet/src/lib.rs
			"submit_beacon_chain_light_client_update",
			vec![self.get_type_chain_argument(), light_client_update.as_value()],
		);

		let tx_hash = self.api.tx().sign_and_submit_default(&tx, &self.signer).await?;
		println!("Submitted tx with hash {tx_hash}");
		Ok(())
	}

	async fn get_finalized_beacon_block_hash(&self) -> Result<eth_types::H256, crate::Error> {
		let extended_beacon_header = self
			.get_value_with_simple_type_chain_argument::<Option<ExtendedBeaconBlockHeader>>(
				"FinalizedBeaconHeader",
			)
			.await?;

		if let Some(extended_beacon_header) = extended_beacon_header {
			Ok(extended_beacon_header.beacon_block_root)
		} else {
			Err(Error::Generic("Unable to obtain ExtendedBeaconBlockHeader").into())
		}
	}

	async fn get_finalized_beacon_block_slot(&self) -> Result<u64, crate::Error> {
		let finalized_beacon_header_value = self
			.get_value_with_simple_type_chain_argument::<Option<ExtendedBeaconBlockHeader>>(
				"FinalizedBeaconHeader",
			)
			.await?;

		if let Some(finalized_beacon_header_value) = finalized_beacon_header_value {
			Ok(finalized_beacon_header_value.header.slot)
		} else {
			Err(Error::Generic("Unable to obtain FinalizedBeaconHeader").into())
		}
	}

	async fn send_headers(
		&mut self,
		headers: &[BlockHeader],
		_end_slot: u64,
	) -> Result<(), crate::Error> {
		let mut txes = vec![];
		for header in headers {
			let tx = subxt::dynamic::tx(
				"Eth2Client",
				"submit_execution_header",
				vec![self.get_type_chain_argument(), header.as_value()],
			);
			txes.push(tx);
		}

		let batch_tx = subxt::dynamic::tx(
			"Utility",
			"batch",
			txes.into_iter().map(|tx| tx.into_value()).collect::<Vec<Value<_>>>(),
		);

		let _tx_hash = self.api.tx().sign_and_submit_default(&batch_tx, &self.signer).await?;
		Ok(())
	}

	async fn get_min_deposit(
		&self,
	) -> Result<crate::eth_client_pallet_trait::Balance, crate::Error> {
		let ret = self
			.get_value_with_simple_type_chain_argument::<Balance>("MinSubmitterBalance")
			.await?;

		Ok(ret)
	}

	async fn register_submitter(&self) -> Result<(), crate::Error> {
		// Create a transaction to submit:
		let tx =
			subxt::dynamic::tx("Eth2Client", "register_submitter", vec![Value::from_bytes(&[])]);

		// submit the transaction with default params:
		let _hash = self.api.tx().sign_and_submit_default(&tx, &self.signer).await?;
		Ok(())
	}

	async fn is_submitter_registered(
		&self,
		account_id: Option<AccountId32>,
	) -> Result<bool, crate::Error> {
		let exists = self
			.get_value_with_keys::<Option<u32>>(
				"Submitters",
				vec![self.get_type_chain_argument(), account_id.as_value()],
			)
			.await
			.map(|r| r.is_some())?;

		Ok(exists)
	}

	async fn get_light_client_state(
		&self,
	) -> Result<eth_types::eth2::LightClientState, crate::Error> {
		let task0 =
			self.get_value_with_simple_type_chain_argument::<Option<_>>("FinalizedBeaconHeader");
		let task1 =
			self.get_value_with_simple_type_chain_argument::<Option<_>>("CurrentSyncCommittee");
		let task2 =
			self.get_value_with_simple_type_chain_argument::<Option<_>>("NextSyncCommittee");

		let (finalized_beacon_header, current_sync_committee, next_sync_committee) =
			tokio::try_join!(task0, task1, task2)?;

		match (finalized_beacon_header, current_sync_committee, next_sync_committee) {
			(
				Some(finalized_beacon_header),
				Some(current_sync_committee),
				Some(next_sync_committee),
			) => Ok(LightClientState {
				finalized_beacon_header,
				current_sync_committee,
				next_sync_committee,
			}),

			_ => Err(Error::Generic("Unable to obtain all values").into()),
		}
	}

	async fn get_num_of_submitted_blocks_by_account(&self) -> Result<u32, crate::Error> {
		let account_id = self.signer.account_id();
		let count = self
			.get_value_with_keys::<Option<u32>>(
				"Submitters",
				vec![self.get_type_chain_argument(), account_id.as_value()],
			)
			.await?
			.ok_or(Error::Generic("Value not found for Submitters"))?;

		Ok(count)
	}

	async fn get_max_submitted_blocks_by_account(&self) -> Result<u32, crate::Error> {
		let ret = self
			.get_value_with_simple_type_chain_argument::<u32>("MaxUnfinalizedBlocksPerSubmitter")
			.await?;

		Ok(ret)
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
			Ok((pair, _)) => Ok(pair),
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
			Ok((pair, _)) => Ok(pair),
			Err(e) => {
				Err(format!("{e:?}").into())
			}
		}
	}
}

#[subxt::subxt(runtime_metadata_path = "../../metadata/eth_light_client_runtime.scale")]
pub mod tangle {}