use async_trait::async_trait;

use crate::eth_client_pallet_trait::EthClientPalletTrait;
use eth_types::{
	eth2::{ExtendedBeaconBlockHeader, LightClientState, LightClientUpdate, SyncCommittee},
	pallet::ClientMode,
	primitives::{FinalExecutionOutcomeView, FinalExecutionStatus},
	BlockHeader, H256,
};
use std::error::Error;
use subxt::utils::AccountId32;
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
		_ => panic!("Unsupported chain id"),
	}
}

pub async fn setup_api() -> Result<OnlineClient<PolkadotConfig>, Box<dyn Error>> {
	let api: OnlineClient<PolkadotConfig> = OnlineClient::<PolkadotConfig>::new().await?;
	Ok(api)
}

pub struct EthClientPallet {
	api: OnlineClient<PolkadotConfig>,
	signer: PairSigner<PolkadotConfig, Pair>,
	chain: TypedChainId,
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
		Self { end_slot: 0, api, signer, chain: typed_chain_id }
	}

	pub fn new_with_suri_key<T: AsRef<str>>(
		api: OnlineClient<PolkadotConfig>,
		suri_key: T,
		typed_chain_id: TypedChainId,
	) -> Result<Self, Box<dyn Error>> {
		let pair = get_sr25519_keys_from_suri(suri_key)?;
		Ok(Self::new_with_pair(api, pair, typed_chain_id))
	}

	pub fn get_signer_account_id(&self) -> AccountId32 {
		(*AsRef::<[u8; 32]>::as_ref(&self.signer.account_id())).into()
	}

	pub async fn is_initialized(
		&self,
		typed_chain_id: TypedChainId,
	) -> Result<bool, Box<dyn Error>> {
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
	) -> Result<(), Box<dyn Error>> {
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

	async fn get_value_or_default<'a, Address: StorageAddress>(
		&self,
		key_addr: &Address,
	) -> Result<Address::Target, Box<dyn Error>>
	where
		Address: StorageAddress<IsFetchable = Yes, IsDefaultable = Yes> + 'a,
	{
		let value = self.api.storage().at_latest().await?.fetch_or_default(key_addr).await?;

		Ok(value)
	}

	async fn get_value<'a, Address: StorageAddress>(
		&self,
		key_addr: &Address,
	) -> Result<Option<Address::Target>, Box<dyn Error>>
	where
		Address: StorageAddress<IsFetchable = Yes> + 'a,
	{
		let value = self.api.storage().at_latest().await?.fetch(key_addr).await?;

		Ok(value)
	}

	async fn submit<Call: TxPayload>(&self, call: &Call) -> Result<H256, Box<dyn Error>> {
		let mut progress =
			self.api.tx().sign_and_submit_then_watch_default(call, &self.signer).await?;

		while let Some(event) = progress.next_item().await {
			let e = match event {
				Ok(e) => e,
				Err(err) => {
					log::error!("failed to watch for tx events {err:?}");
					return Err(Box::new(std::io::Error::new(
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
							return Err(Box::new(std::io::Error::new(
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

		Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Transaction stream ended")))
	}
}

#[async_trait]
impl EthClientPalletTrait for EthClientPallet {
	async fn send_light_client_update(
		&mut self,
		light_client_update: LightClientUpdate,
	) -> Result<FinalExecutionOutcomeView<Box<dyn Error>>, Box<dyn Error>> {
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

	async fn get_finalized_beacon_block_hash(&self) -> Result<eth_types::H256, Box<dyn Error>> {
		let addr = tangle::storage()
			.eth2_client()
			.finalized_beacon_header(convert_typed_chain_ids(self.chain));
		if let Some(header) = self.get_value(&addr).await? {
			Ok(eth_types::H256::from(header.beacon_block_root.0))
		} else {
			Err(Box::new(std::io::Error::new(
				std::io::ErrorKind::Other,
				"Unable to get value for get_finalized_beacon_block_hash".to_string(),
			)))
		}
	}

	async fn get_finalized_beacon_block_slot(&self) -> Result<u64, Box<dyn Error>> {
		let addr = tangle::storage()
			.eth2_client()
			.finalized_beacon_header(convert_typed_chain_ids(self.chain));
		if let Some(header) = self.get_value(&addr).await? {
			Ok(header.header.slot)
		} else {
			Err(Box::new(std::io::Error::new(
				std::io::ErrorKind::Other,
				"Unable to get value for get_finalized_beacon_block_slot".to_string(),
			)))
		}
	}

	async fn send_headers(
		&mut self,
		headers: &[BlockHeader],
	) -> Result<FinalExecutionOutcomeView<Box<dyn Error>>, Box<dyn Error>> {
		if headers.is_empty() {
			return Err(Box::new(std::io::Error::new(
				std::io::ErrorKind::Other,
				"Tried to submit empty headers".to_string(),
			)))
		}

		let mut txes = vec![];
		for header in headers {
			let decoded_tcid = Decode::decode(&mut self.chain.encode().as_slice()).unwrap();
			let decoded_header = Decode::decode(&mut header.encode().as_slice()).unwrap();
			let call = pallet_eth2_light_client::pallet::Call::submit_execution_header {
				typed_chain_id: decoded_tcid,
				block_header: decoded_header,
			};
			let tx = tangle::runtime_types::node_template_runtime::RuntimeCall::Eth2Client(call);
			txes.push(tx);
		}

		let batch_call = tangle::tx().utility().force_batch(txes);

		let tx_hash = self.submit(&batch_call).await?;
		Ok(FinalExecutionOutcomeView {
			status: FinalExecutionStatus::Success,
			transaction_hash: Some(tx_hash),
		})
	}

	async fn get_light_client_state(
		&self,
	) -> Result<eth_types::eth2::LightClientState, Box<dyn Error>> {
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

			_ => Err(Box::new(std::io::Error::new(
				std::io::ErrorKind::Other,
				"Unable to obtain all three values".to_string(),
			))),
		}
	}

	async fn get_client_mode(&self) -> Result<ClientMode, Box<dyn Error>> {
		Err(Box::new(std::io::Error::new(
			std::io::ErrorKind::Other,
			"Unable to get value for get_client_mode".to_string(),
		)))
	}

	async fn get_last_block_number(&self) -> Result<u64, Box<dyn Error>> {
		Err(Box::new(std::io::Error::new(
			std::io::ErrorKind::Other,
			"Unable to get value for get_last_block_number".to_string(),
		)))
	}
	async fn get_unfinalized_tail_block_number(&self) -> Result<Option<u64>, Box<dyn Error>> {
		Err(Box::new(std::io::Error::new(
			std::io::ErrorKind::Other,
			"Unable to get value for get_unfinalized_tail_block_number".to_string(),
		)))
	}
}

fn get_sr25519_keys_from_suri<T: AsRef<str>>(suri: T) -> Result<Pair, Box<dyn Error>> {
	let value = suri.as_ref();
	if value.starts_with('$') {
		// env
		let var = value.strip_prefix('$').unwrap_or(value);
		log::trace!("Reading {} from env", var);
		let val = std::env::var(var).map_err(|e| {
			Box::new(std::io::Error::new(
				std::io::ErrorKind::Other,
				format!("error while loading this env {var}: {e}"),
			))
		})?;
		let maybe_pair = Pair::from_string(&val, None);
		match maybe_pair {
			Ok(pair) => Ok(pair),
			Err(e) =>
				Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("{e:?}")))),
		}
	} else if value.starts_with('>') {
		todo!("Implement command execution to extract the private key")
	} else {
		let maybe_pair = Pair::from_string(value, None);
		match maybe_pair {
			Ok(pair) => Ok(pair),
			Err(e) =>
				Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("{e:?}")))),
		}
	}
}

#[subxt::subxt(runtime_metadata_path = "../../metadata/eth_light_client_runtime.scale")]
pub mod tangle {}
