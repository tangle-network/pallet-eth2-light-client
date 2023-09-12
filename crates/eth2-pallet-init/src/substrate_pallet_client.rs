use async_trait::async_trait;

use crate::eth_client_pallet_trait::EthClientPalletTrait;
use eth_types::{
	eth2::{ExtendedBeaconBlockHeader, LightClientState, LightClientUpdate, SyncCommittee},
	pallet::ClientMode,
	primitives::{FinalExecutionOutcomeView, FinalExecutionStatus},
	BlockHeader, H256,
};

use subxt::{error::DispatchError, utils::AccountId32};
use webb::substrate::{
	scale::{Decode, Encode},
	subxt::{
		self,
		ext::sp_core::{sr25519::Pair, Pair as _},
		storage::{address::Yes, StorageAddress},
		tx::{PairSigner, TxPayload, TxStatus},
		OnlineClient, PolkadotConfig,
	},
};
use webb_proposals::TypedChainId;

use tangle::runtime_types::pallet_eth2_light_client;

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
		_ => unimplemented!("Unsupported chain id"),
	}
}

pub fn convert_mode(t: tangle::runtime_types::eth_types::pallet::ClientMode) -> ClientMode {
	match t {
		tangle::runtime_types::eth_types::pallet::ClientMode::SubmitLightClientUpdate =>
			ClientMode::SubmitLightClientUpdate,
		tangle::runtime_types::eth_types::pallet::ClientMode::SubmitHeader =>
			ClientMode::SubmitHeader,
	}
}

pub async fn setup_api() -> anyhow::Result<OnlineClient<PolkadotConfig>> {
	let api: OnlineClient<PolkadotConfig> = OnlineClient::<PolkadotConfig>::new().await?;
	Ok(api)
}

#[derive(Clone)]
pub struct EthClientPallet {
	api: OnlineClient<PolkadotConfig>,
	pair: Pair,
	chain: TypedChainId,
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
		Self { api, pair, chain: typed_chain_id }
	}

	pub fn new_with_suri_key<T: AsRef<str>>(
		api: OnlineClient<PolkadotConfig>,
		suri_key: T,
		typed_chain_id: TypedChainId,
	) -> anyhow::Result<Self> {
		let pair = get_sr25519_keys_from_suri(suri_key)?;
		Ok(Self::new_with_pair(api, pair, typed_chain_id))
	}

	pub fn get_signer_account_id(&self) -> AccountId32 {
		let signer: PairSigner<PolkadotConfig, Pair> = PairSigner::new(self.pair.clone());
		(*AsRef::<[u8; 32]>::as_ref(&signer.account_id())).into()
	}

	pub async fn is_initialized(&self, typed_chain_id: TypedChainId) -> anyhow::Result<bool> {
		let address = tangle::storage()
			.eth2_client()
			.finalized_beacon_header(convert_typed_chain_ids(typed_chain_id));
		self.get_value(&address).await.map(|x| x.is_some())
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
		trusted_signer: Option<AccountId32>,
	) -> anyhow::Result<()> {
		self.chain = typed_chain_id;

		let trusted_signer = if let Some(trusted_signer) = trusted_signer {
			let bytes: [u8; 32] = *trusted_signer.as_ref();
			Some(subxt::utils::AccountId32::from(bytes))
		} else {
			None
		};

		let init_input = tangle::runtime_types::eth_types::pallet::InitInput {
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
			trusted_signer,
		};

		let tx = tangle::tx()
			.eth2_client()
			.init(convert_typed_chain_ids(typed_chain_id), init_input);

		// submit the transaction with default params:
		let _hash = self.submit(&tx).await?;

		Ok(())
	}

	#[allow(dead_code)]
	async fn get_value_or_default<'a, Address: StorageAddress>(
		&self,
		key_addr: &Address,
	) -> anyhow::Result<Address::Target>
	where
		Address: StorageAddress<IsFetchable = Yes, IsDefaultable = Yes> + 'a,
	{
		let value = self.api.storage().at_latest().await?.fetch_or_default(key_addr).await?;

		Ok(value)
	}

	async fn get_value<'a, Address: StorageAddress>(
		&self,
		key_addr: &Address,
	) -> anyhow::Result<Option<Address::Target>>
	where
		Address: StorageAddress<IsFetchable = Yes> + 'a,
	{
		let value = self.api.storage().at_latest().await?.fetch(key_addr).await?;

		Ok(value)
	}

	async fn submit<Call: TxPayload>(&self, call: &Call) -> anyhow::Result<H256> {
		let signer = PairSigner::new(self.pair.clone());
		let mut progress = self.api.tx().sign_and_submit_then_watch_default(call, &signer).await?;

		while let Some(event) = progress.next_item().await {
			let e = match event {
				Ok(e) => e,
				Err(err) => {
					log::error!("failed to watch for tx events {err:?}");
					return Err(std::io::Error::new(
						std::io::ErrorKind::Other,
						format!("Failed to get hash storage value: {err:?}"),
					)
					.into())
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
							let error_msg = match err {
								subxt::Error::Runtime(DispatchError::Module(error)) => {
									let details = error.details()?;
									let pallet = details.pallet.name();
									let error = &details.variant;
									format!("Extrinsic failed with an error: {pallet}::{error:?}")
								},
								_ => {
									format!("Extrinsic failed with an error: {}", err)
								},
							};

							return Err(std::io::Error::new(
								std::io::ErrorKind::Other,
								format!("Tx failed : {error_msg}"),
							)
							.into())
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

		Err(std::io::Error::new(std::io::ErrorKind::Other, "Transaction stream ended").into())
	}
}

#[async_trait]
impl EthClientPalletTrait for EthClientPallet {
	async fn send_light_client_update(
		&mut self,
		light_client_update: LightClientUpdate,
	) -> anyhow::Result<FinalExecutionOutcomeView> {
		let decoded_lcu = Decode::decode(&mut light_client_update.encode().as_slice()).unwrap();
		let decoded_tcid = Decode::decode(&mut self.chain.encode().as_slice()).unwrap();
		let call = tangle::tx()
			.eth2_client()
			.submit_beacon_chain_light_client_update(decoded_tcid, decoded_lcu);
		let tx_hash = self.submit(&call).await?;
		Ok(FinalExecutionOutcomeView {
			status: FinalExecutionStatus::Success,
			transaction_hash: Some(tx_hash),
		})
	}

	async fn get_finalized_beacon_block_hash(&self) -> anyhow::Result<eth_types::H256> {
		let addr = tangle::storage()
			.eth2_client()
			.finalized_beacon_header(convert_typed_chain_ids(self.chain));
		if let Some(header) = self.get_value(&addr).await? {
			Ok(eth_types::H256::from(header.beacon_block_root.0))
		} else {
			Err(std::io::Error::new(
				std::io::ErrorKind::Other,
				"Unable to get value for get_finalized_beacon_block_hash".to_string(),
			)
			.into())
		}
	}

	async fn get_finalized_beacon_block_slot(&self) -> anyhow::Result<u64> {
		let addr = tangle::storage()
			.eth2_client()
			.finalized_beacon_header(convert_typed_chain_ids(self.chain));
		if let Some(header) = self.get_value(&addr).await? {
			Ok(header.header.slot)
		} else {
			Err(std::io::Error::new(
				std::io::ErrorKind::Other,
				"Unable to get value for get_finalized_beacon_block_slot".to_string(),
			)
			.into())
		}
	}

	async fn send_headers(
		&mut self,
		headers: &[BlockHeader],
	) -> anyhow::Result<FinalExecutionOutcomeView> {
		if headers.is_empty() {
			return Err(std::io::Error::new(
				std::io::ErrorKind::Other,
				"Tried to submit empty headers".to_string(),
			)
			.into())
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

		let tx_hash = self.submit(&batch_call).await?;
		Ok(FinalExecutionOutcomeView {
			status: FinalExecutionStatus::Success,
			transaction_hash: Some(tx_hash),
		})
	}

	async fn get_light_client_state(&self) -> anyhow::Result<eth_types::eth2::LightClientState> {
		let addr0 = tangle::storage()
			.eth2_client()
			.finalized_beacon_header(convert_typed_chain_ids(self.chain));
		let finalized_beacon_header = self.get_value(&addr0).await?;

		let addr1 = tangle::storage()
			.eth2_client()
			.current_sync_committee(convert_typed_chain_ids(self.chain));
		let current_sync_committee = self.get_value(&addr1).await?;

		let addr2 = tangle::storage()
			.eth2_client()
			.next_sync_committee(convert_typed_chain_ids(self.chain));
		let next_sync_committee = self.get_value(&addr2).await?;

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

			_ => Err(std::io::Error::new(
				std::io::ErrorKind::Other,
				"Unable to obtain all three values".to_string(),
			)
			.into()),
		}
	}

	async fn get_client_mode(&self) -> anyhow::Result<ClientMode> {
		let addr = tangle::storage()
			.eth2_client()
			.client_mode_for_chain(convert_typed_chain_ids(self.chain));

		if let Some(mode) = self.get_value(&addr).await? {
			Ok(convert_mode(mode))
		} else {
			Ok(ClientMode::SubmitLightClientUpdate)
		}
	}

	async fn get_last_block_number(&self) -> anyhow::Result<u64> {
		let addr = tangle::storage()
			.eth2_client()
			.finalized_execution_header(convert_typed_chain_ids(self.chain));

		if let Some(head) = self.get_value(&addr).await? {
			Ok(head.block_number)
		} else {
			Ok(0)
		}
	}

	async fn get_unfinalized_tail_block_number(&self) -> anyhow::Result<Option<u64>> {
		let addr = tangle::storage()
			.eth2_client()
			.unfinalized_tail_execution_header(convert_typed_chain_ids(self.chain));

		if let Some(head) = self.get_value(&addr).await? {
			Ok(Some(head.block_number))
		} else {
			Ok(None)
		}
	}
}

fn get_sr25519_keys_from_suri<T: AsRef<str>>(suri: T) -> anyhow::Result<Pair> {
	let value = suri.as_ref();
	if value.starts_with('$') {
		// env
		let var = value.strip_prefix('$').unwrap_or(value);
		log::trace!("Reading {} from env", var);
		let val = std::env::var(var).map_err(|e| {
			std::io::Error::new(
				std::io::ErrorKind::Other,
				format!("error while loading this env {var}: {e}"),
			)
		})?;
		let maybe_pair = Pair::from_string(&val, None);
		match maybe_pair {
			Ok(pair) => Ok(pair),
			Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, format!("{e:?}")).into()),
		}
	} else if value.starts_with('>') {
		todo!("Implement command execution to extract the private key")
	} else {
		let maybe_pair = Pair::from_string(value, None);
		match maybe_pair {
			Ok(pair) => Ok(pair),
			Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, format!("{e:?}")).into()),
		}
	}
}

#[subxt::subxt(runtime_metadata_path = "./metadata/tangle-runtime.scale")]
pub mod tangle {}
