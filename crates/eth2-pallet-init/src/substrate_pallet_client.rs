use async_trait::async_trait;

use eth_types::{
	eth2::{ExtendedBeaconBlockHeader, LightClientState, LightClientUpdate, SyncCommittee},
	BlockHeader, H256,
};
use sp_core::{sr25519::Pair, Pair as PairT};
use subxt::utils::AccountId32;
use webb::substrate::{
	scale::{Decode, Encode},
	subxt::{
		self,
		metadata::DecodeWithMetadata,
		storage::{address::Yes, StorageAddress},
		tx::{PairSigner, TxPayload, TxStatus},
		OnlineClient, PolkadotConfig,
	},
};
use webb_proposals::TypedChainId;
use webb_relayer_utils::Error;

use crate::eth_client_pallet_trait::EthClientPalletTrait;

use self::tangle::runtime_types::pallet_eth2_light_client;

pub fn convert_typed_chain_ids(
	t: TypedChainId,
) -> tangle::runtime_types::webb_proposals::header::TypedChainId {
	match t {
		TypedChainId::None => tangle::runtime_types::webb_proposals::header::TypedChainId::None,
		TypedChainId::Evm(id) =>
			tangle::runtime_types::webb_proposals::header::TypedChainId::Evm(id),
		TypedChainId::Substrate(id) =>
			tangle::runtime_types::webb_proposals::header::TypedChainId::Substrate(id),
		TypedChainId::PolkadotParachain(id) =>
			tangle::runtime_types::webb_proposals::header::TypedChainId::PolkadotParachain(id),
		TypedChainId::KusamaParachain(id) =>
			tangle::runtime_types::webb_proposals::header::TypedChainId::KusamaParachain(id),
		TypedChainId::RococoParachain(id) =>
			tangle::runtime_types::webb_proposals::header::TypedChainId::RococoParachain(id),
		TypedChainId::Cosmos(id) =>
			tangle::runtime_types::webb_proposals::header::TypedChainId::Cosmos(id),
		TypedChainId::Solana(id) =>
			tangle::runtime_types::webb_proposals::header::TypedChainId::Solana(id),
		TypedChainId::Ink(id) =>
			tangle::runtime_types::webb_proposals::header::TypedChainId::Ink(id),
	}
}

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
	max_submitted_blocks_by_account: Option<u32>,
	end_slot: u64,
}

impl EthClientPallet {
	pub fn new(api: OnlineClient<PolkadotConfig>, typed_chain_id: TypedChainId) -> Self {
		Self::new_with_pair(api, sp_keyring::AccountKeyring::Alice.pair(), typed_chain_id)
	}

	pub fn new_with_pair(
		api: OnlineClient<PolkadotConfig>,
		pair: Pair,
		typed_chain_id: TypedChainId,
	) -> Self {
		let signer = PairSigner::new(pair);
		// set to defaults. These values will change later in init
		Self {
			end_slot: 0,
			api,
			signer,
			chain: convert_typed_chain_ids(typed_chain_id),
			max_submitted_blocks_by_account: None,
		}
	}

	pub fn new_with_suri_key<T: AsRef<str>>(
		api: OnlineClient<PolkadotConfig>,
		suri_key: T,
		typed_chain_id: TypedChainId,
	) -> Result<Self, crate::Error> {
		let pair = get_sr25519_keys_from_suri(suri_key)?;
		Ok(Self::new_with_pair(api, pair, typed_chain_id))
	}

	pub fn get_signer_account_id(&self) -> AccountId32 {
		(*AsRef::<[u8; 32]>::as_ref(&self.signer.account_id())).into()
	}

	pub async fn is_initialized(&self, typed_chain_id: TypedChainId) -> Result<bool, Error> {
		let address = tangle::storage()
			.eth2_client()
			.finalized_beacon_header(convert_typed_chain_ids(typed_chain_id));
		self.get_value(&address).await.map(|x| x.is_some()).map_err(|err| {
			Error::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("{err:?}")))
		})
	}

	#[allow(clippy::too_many_arguments)]
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
		self.chain = Decode::decode(&mut typed_chain_id.clone().encode().as_slice()).unwrap();

		let trusted_signer = if let Some(trusted_signer) = trusted_signer {
			let bytes: [u8; 32] = *trusted_signer.as_ref();
			Some(subxt::utils::AccountId32::from(bytes))
		} else {
			None
		};

		let init_input: tangle::runtime_types::eth_types::pallet::InitInput<
			subxt::utils::AccountId32,
		> = tangle::runtime_types::eth_types::pallet::InitInput {
			finalized_execution_header: Decode::decode(
				&mut finalized_execution_header.encode().as_slice(),
			)?,
			finalized_beacon_header: Decode::decode(
				&mut finalized_beacon_header.encode().as_slice(),
			)?,
			current_sync_committee: Decode::decode(
				&mut current_sync_committee.encode().as_slice(),
			)?,
			next_sync_committee: Decode::decode(&mut next_sync_committee.encode().as_slice())?,
			validate_updates: validate_updates.unwrap_or(true),
			verify_bls_signatures: verify_bls_signatures.unwrap_or(true),
			hashes_gc_threshold: hashes_gc_threshold.unwrap_or(100),
			max_submitted_blocks_by_account,
			trusted_signer,
		};

		let tx = tangle::tx()
			.eth2_client()
			.init(convert_typed_chain_ids(typed_chain_id), init_input);

		// submit the transaction with default params:
		let _hash = self.submit(&tx).await.map_err(|err| {
			Error::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("{err:?}")))
		})?;

		Ok(())
	}

	async fn get_value_or_default<'a, Address: StorageAddress>(
		&self,
		key_addr: &Address,
	) -> Result<<Address::Target as DecodeWithMetadata>::Target, crate::Error>
	where
		Address: StorageAddress<IsFetchable = Yes, IsDefaultable = Yes> + 'a,
	{
		let value =
			self.api
				.storage()
				.at(None)
				.await?
				.fetch_or_default(key_addr)
				.await
				.map_err(|err| {
					Error::Io(std::io::Error::new(
						std::io::ErrorKind::Other,
						format!("Failed to get api storage value: {err:?}"),
					))
				})?;

		Ok(value)
	}

	async fn get_value<'a, Address: StorageAddress>(
		&self,
		key_addr: &Address,
	) -> Result<Option<<Address::Target as DecodeWithMetadata>::Target>, crate::Error>
	where
		Address: StorageAddress<IsFetchable = Yes> + 'a,
	{
		let value = self.api.storage().at(None).await?.fetch(key_addr).await.map_err(|err| {
			Error::Io(std::io::Error::new(
				std::io::ErrorKind::Other,
				format!("Failed to get api storage value: {err:?}"),
			))
		})?;

		Ok(value)
	}

	async fn submit<Call: TxPayload>(&self, call: &Call) -> Result<H256, crate::Error> {
		let mut progress = self
			.api
			.tx()
			.sign_and_submit_then_watch_default(call, &self.signer)
			.await
			.map_err(|err| {
				Error::Io(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("Failed to get hash storage value: {err:?}"),
				))
			})?;

		while let Some(event) = progress.next_item().await {
			let e = match event {
				Ok(e) => e,
				Err(err) => {
					log::error!("failed to watch for tx events {err:?}");
					return Err(crate::Error::from(std::io::Error::new(
						std::io::ErrorKind::Other,
						format!("Failed to get hash storage value: {err:?}"),
					)))
				},
			};

			match e {
				TxStatus::Future => {},
				TxStatus::Ready => {
					log::trace!("tx ready");
				},
				TxStatus::Broadcast(_) => {},
				TxStatus::InBlock(_) => {
					log::trace!("tx in block");
				},
				TxStatus::Retracted(_) => {
					log::warn!("tx retracted");
				},
				TxStatus::FinalityTimeout(_) => {
					log::warn!("tx timeout");
				},
				TxStatus::Finalized(v) => {
					let maybe_success = v.wait_for_success().await;
					match maybe_success {
						Ok(events) => {
							log::debug!("tx finalized");
							let hash = events.extrinsic_hash();
							return Ok(hash.0.into())
						},
						Err(err) => {
							log::error!("tx failed {err:?}");
							return Err(crate::Error::from(std::io::Error::new(
								std::io::ErrorKind::Other,
								format!("Failed to get hash storage value: {err:?}"),
							)))
						},
					}
				},
				TxStatus::Usurped(_) => {},
				TxStatus::Dropped => {
					log::warn!("tx dropped");
				},
				TxStatus::Invalid => {
					log::warn!("tx invalid");
				},
			}
		}

		Err(crate::Error::from(std::io::Error::new(
			std::io::ErrorKind::Other,
			"Transaction stream ended",
		)))
	}
}

#[async_trait]
impl EthClientPalletTrait for EthClientPallet {
	async fn get_last_submitted_slot(&self) -> Result<u64, crate::Error> {
		self.get_finalized_beacon_block_slot().await
	}

	async fn is_known_block(
		&self,
		execution_block_hash: &eth_types::H256,
	) -> Result<bool, crate::Error> {
		let decoded: tangle::runtime_types::eth_types::H256 =
			Decode::decode(&mut execution_block_hash.encode().as_slice()).unwrap();
		let addr = tangle::storage().eth2_client().unfinalized_headers(&self.chain, decoded);
		self.get_value(&addr).await.map(|r| r.is_some())
	}

	async fn send_light_client_update(
		&mut self,
		light_client_update: LightClientUpdate,
	) -> Result<(), crate::Error> {
		let decoded_lcu = Decode::decode(&mut light_client_update.encode().as_slice()).unwrap();
		let decoded_tcid = Decode::decode(&mut self.chain.encode().as_slice()).unwrap();
		let call = tangle::tx()
			.eth2_client()
			.submit_beacon_chain_light_client_update(decoded_tcid, decoded_lcu);
		self.submit(&call).await.map(|_| ())
	}

	async fn get_finalized_beacon_block_hash(&self) -> Result<eth_types::H256, crate::Error> {
		let addr = tangle::storage().eth2_client().finalized_beacon_header(&self.chain);
		if let Some(header) = self.get_value(&addr).await? {
			Ok(eth_types::H256::from(header.beacon_block_root.0))
		} else {
			Err(crate::Error::from("Unable to get value for get_finalized_beacon_block_hash"))
		}
	}

	async fn get_finalized_beacon_block_slot(&self) -> Result<u64, crate::Error> {
		let addr = tangle::storage().eth2_client().finalized_beacon_header(&self.chain);
		if let Some(header) = self.get_value(&addr).await? {
			Ok(header.header.slot)
		} else {
			Err(crate::Error::from("Unable to get value for get_finalized_beacon_block_slot"))
		}
	}

	async fn send_headers(
		&mut self,
		headers: &[BlockHeader],
		end_slot: u64,
	) -> Result<(), crate::Error> {
		self.end_slot = end_slot;
		if headers.is_empty() {
			return Err(crate::Error::from("Tried to submit zero headers"))
		}

		let mut txes = vec![];
		for header in headers {
			let decoded_tcid = Decode::decode(&mut self.chain.encode().as_slice()).unwrap();
			let decoded_header = Decode::decode(&mut header.encode().as_slice()).unwrap();
			let call = pallet_eth2_light_client::pallet::Call::submit_execution_header {
				typed_chain_id: decoded_tcid,
				block_header: decoded_header,
			};
			let tx =
				tangle::runtime_types::tangle_standalone_runtime::RuntimeCall::Eth2Client(call);
			txes.push(tx);
		}

		let batch_call = tangle::tx().utility().force_batch(txes);

		self.submit(&batch_call).await.map(|_| ())
	}

	async fn get_min_deposit(
		&self,
	) -> Result<crate::eth_client_pallet_trait::Balance, crate::Error> {
		let addr = tangle::storage().eth2_client().min_submitter_balance(&self.chain);
		self.get_value_or_default(&addr).await
	}

	async fn register_submitter(&self) -> Result<(), crate::Error> {
		log::info!(target: "relay", "About to register the submitter ...");
		let decoded_tcid = Decode::decode(&mut self.chain.encode().as_slice()).unwrap();
		let tx = tangle::tx().eth2_client().register_submitter(decoded_tcid);
		self.submit(&tx).await.map(|_| ())
	}

	async fn is_submitter_registered(
		&self,
		account_id: Option<AccountId32>,
	) -> Result<bool, crate::Error> {
		let account_id = account_id.unwrap_or(self.get_signer_account_id());
		log::info!(target: "relay", "Attempting to see if {account_id:?} is registered ...");

		let bytes: [u8; 32] = *account_id.as_ref();
		let addr = tangle::storage()
			.eth2_client()
			.submitters(&self.chain, subxt::utils::AccountId32::from(bytes));
		self.get_value(&addr).await.map(|r| r.is_some())
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
			(
				Some(finalized_beacon_header),
				Some(current_sync_committee),
				Some(next_sync_committee),
			) => Ok(LightClientState {
				finalized_beacon_header: Decode::decode(
					&mut finalized_beacon_header.encode().as_slice(),
				)
				.unwrap(),
				current_sync_committee: Decode::decode(
					&mut current_sync_committee.encode().as_slice(),
				)
				.unwrap(),
				next_sync_committee: Decode::decode(&mut next_sync_committee.encode().as_slice())
					.unwrap(),
			}),

			_ => Err(crate::Error::from("Unable to obtain all three values")),
		}
	}

	async fn get_num_of_submitted_blocks_by_account(&self) -> Result<u32, crate::Error> {
		let account_id = self.signer.account_id();
		let addr = tangle::storage().eth2_client().submitters(&self.chain, account_id);
		if let Some(val) = self.get_value(&addr).await? {
			Ok(val)
		} else {
			log::warn!("Failed to get value for get_num_of_submitted_blocks_by_account (no blocks submitted yet?)");
			Ok(0)
		}
	}

	async fn get_max_submitted_blocks_by_account(&self) -> Result<u32, crate::Error> {
		let key_addr = tangle::storage()
			.eth2_client()
			.max_unfinalized_blocks_per_submitter(&self.chain);

		let value: u32 =
			self.api.storage().at(None).await?.fetch_or_default(&key_addr).await.map_err(
				|err| {
					Error::Io(std::io::Error::new(
						std::io::ErrorKind::Other,
						format!("Failed to get api storage value: {err:?}"),
					))
				},
			)?;

		Ok(value)
	}
}

fn get_sr25519_keys_from_suri<T: AsRef<str>>(suri: T) -> Result<Pair, crate::Error> {
	let value = suri.as_ref();
	if value.starts_with('$') {
		// env
		let var = value.strip_prefix('$').unwrap_or(value);
		log::trace!("Reading {} from env", var);
		let val = std::env::var(var)
			.map_err(|e| crate::Error::from(format!("error while loading this env {var}: {e}",)))?;
		let maybe_pair = Pair::from_string(&val, None);
		match maybe_pair {
			Ok(pair) => Ok(pair),
			Err(e) => Err(format!("{e:?}").into()),
		}
	} else if value.starts_with('>') {
		todo!("Implement command execution to extract the private key")
	} else {
		let maybe_pair = Pair::from_string(value, None);
		match maybe_pair {
			Ok(pair) => Ok(pair),
			Err(e) => Err(format!("{e:?}").into()),
		}
	}
}

#[subxt::subxt(runtime_metadata_path = "../../metadata/eth_light_client_runtime.scale")]
pub mod tangle {}
