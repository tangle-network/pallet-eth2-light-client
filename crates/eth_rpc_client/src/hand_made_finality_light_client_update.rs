use crate::{
	beacon_block_body_merkle_tree::BeaconStateMerkleTree,
	beacon_rpc_client::BeaconRPCClient,
	errors::{ErrorOnUnwrapSignatureBit, MissNextSyncCommittee, MissSyncAggregationError},
	execution_block_proof::ExecutionBlockProof,
};
use eth_types::{
	eth2::{
		FinalizedHeaderUpdate, HeaderUpdate, LightClientUpdate, SignatureBytes, SyncCommittee,
		SyncCommitteeBits, SyncCommitteeUpdate,
	},
	H256,
};
use funty::Fundamental;
use log::trace;
use serde_json::Value;
use ssz::Encode;
use tree_hash::TreeHash;
use types::{BeaconBlockBody, BeaconBlockHeader, BeaconState, MainnetEthSpec, Slot};

pub struct HandMadeFinalityLightClientUpdate {}

impl HandMadeFinalityLightClientUpdate {
	pub async fn get_finality_light_client_update(
		beacon_rpc_client: &BeaconRPCClient,
		attested_slot: u64,
		include_next_sync_committee: bool,
	) -> Result<LightClientUpdate, crate::Error> {
		let (attested_slot, signature_slot) =
			Self::get_attested_slot_with_enough_sync_committee_bits_sum(
				beacon_rpc_client,
				attested_slot,
			)
			.await?;
		trace!(target: "relay", "New attested slot = {} and signature slot = {}", attested_slot, signature_slot);
		let beacon_state = beacon_rpc_client.get_beacon_state(&format!("{attested_slot}")).await?;

		let finality_hash = beacon_state.finalized_checkpoint().root;
		let finality_header = beacon_rpc_client
			.get_beacon_block_header_for_block_id(&format!("{:?}", &finality_hash))
			.await?;
		let finality_slot = finality_header.slot.as_u64();

		let finality_beacon_state = match include_next_sync_committee {
			true => Some(beacon_rpc_client.get_beacon_state(&format!("{finality_slot}")).await?),
			false => None,
		};

		Self::get_finality_light_client_update_for_state(
			beacon_rpc_client,
			attested_slot,
			signature_slot,
			beacon_state,
			finality_beacon_state,
		)
		.await
	}

	pub async fn get_finality_light_client_update_from_file(
		beacon_rpc_client: &BeaconRPCClient,
		file_name: &str,
	) -> Result<LightClientUpdate, crate::Error> {
		let beacon_state = Self::get_state_from_file(file_name)?;
		let attested_slot = beacon_state.slot().as_u64();

		let signature_slot = beacon_rpc_client
			.get_non_empty_beacon_block_header(attested_slot + 1)
			.await?
			.slot;

		Self::get_finality_light_client_update_for_state(
			beacon_rpc_client,
			attested_slot,
			signature_slot,
			beacon_state,
			None,
		)
		.await
	}

	pub async fn get_light_client_update_from_file_with_next_sync_committee(
		beacon_rpc_client: &BeaconRPCClient,
		attested_state_file_name: &str,
		finality_state_file_name: &str,
	) -> Result<LightClientUpdate, crate::Error> {
		let attested_beacon_state = Self::get_state_from_file(attested_state_file_name)?;
		let attested_slot = attested_beacon_state.slot().as_u64();
		let finality_beacon_state = Self::get_state_from_file(finality_state_file_name)?;
		let signature_slot = beacon_rpc_client
			.get_non_empty_beacon_block_header(attested_slot + 1)
			.await?
			.slot;

		Self::get_finality_light_client_update_for_state(
			beacon_rpc_client,
			attested_slot,
			signature_slot,
			attested_beacon_state,
			Some(finality_beacon_state),
		)
		.await
	}
}

impl HandMadeFinalityLightClientUpdate {
	async fn get_attested_slot_with_enough_sync_committee_bits_sum(
		beacon_rpc_client: &BeaconRPCClient,
		attested_slot: u64,
	) -> Result<(u64, u64), crate::Error> {
		let mut current_attested_slot = attested_slot;
		loop {
			let signature_slot = beacon_rpc_client
				.get_non_empty_beacon_block_header(current_attested_slot + 1)
				.await?
				.slot;
			let signature_beacon_body = beacon_rpc_client
				.get_beacon_block_body_for_block_id(&format!("{signature_slot}"))
				.await?;
			let sync_aggregate =
				signature_beacon_body.sync_aggregate().map_err(|_| MissSyncAggregationError)?;
			let sync_committee_bits: [u8; 64] = Self::get_sync_committee_bits(sync_aggregate)?;
			let sync_committee_bits_sum: u32 =
				sync_committee_bits.into_iter().map(|x| x.count_ones()).sum();
			if sync_committee_bits_sum * 3 < (64 * 8 * 2) {
				current_attested_slot = signature_slot;
				continue
			}

			if signature_beacon_body.attestations().is_empty() {
				current_attested_slot = signature_slot;
				continue
			}

			let mut attested_slots: Vec<u64> = signature_beacon_body
				.attestations()
				.into_iter()
				.map(|attestation| attestation.data.slot.as_u64())
				.collect();
			attested_slots.sort();

			for i in (0..attested_slots.len()).rev() {
				if (i == attested_slots.len() - 1 || attested_slots[i + 1] != attested_slots[i]) &&
					attested_slots[i] >= attested_slot
				{
					current_attested_slot = attested_slots[i];

					if let Err(err) = beacon_rpc_client
						.get_beacon_block_header_for_block_id(&format!("{current_attested_slot}"))
						.await
					{
						if err.is_no_block_for_slot_error.is_none() {
							return Err(err)
						}
					} else {
						return Ok((current_attested_slot, signature_slot))
					}
				}
			}

			current_attested_slot = signature_slot;
		}
	}

	fn get_state_from_file(file_name: &str) -> Result<BeaconState<MainnetEthSpec>, crate::Error> {
		let beacon_state_json: String =
			std::fs::read_to_string(file_name).expect("Unable to read file");

		let v: Value = serde_json::from_str(&beacon_state_json)?;
		let beacon_state_json = serde_json::to_string(&v["data"])?;

		Ok(serde_json::from_str(&beacon_state_json)?)
	}

	async fn get_finality_light_client_update_for_state(
		beacon_rpc_client: &BeaconRPCClient,
		attested_slot: u64,
		signature_slot: u64,
		beacon_state: BeaconState<MainnetEthSpec>,
		finality_beacon_state: Option<BeaconState<MainnetEthSpec>>,
	) -> Result<LightClientUpdate, crate::Error> {
		let signature_beacon_body = beacon_rpc_client
			.get_beacon_block_body_for_block_id(&format!("{signature_slot}"))
			.await?;
		let sync_aggregate =
			signature_beacon_body.sync_aggregate().map_err(|_| MissSyncAggregationError)?;
		let sync_committee_bits: [u8; 64] = Self::get_sync_committee_bits(sync_aggregate)?;

		let attested_header = beacon_rpc_client
			.get_beacon_block_header_for_block_id(&format!("{attested_slot}"))
			.await?;

		let finality_hash = beacon_state.finalized_checkpoint().root;
		let finality_header = beacon_rpc_client
			.get_beacon_block_header_for_block_id(&format!("{:?}", &finality_hash))
			.await?;

		let finalized_block_body = beacon_rpc_client
			.get_beacon_block_body_for_block_id(&format!("{:?}", &finality_hash))
			.await?;

		Ok(LightClientUpdate {
			attested_beacon_header: Self::from_lighthouse_beacon_header(&BeaconBlockHeader {
				slot: Slot::new(attested_header.slot),
				proposer_index: attested_header.proposer_index,
				parent_root: types::Hash256::from_slice(&attested_header.parent_root.0 .0),
				state_root: types::Hash256::from_slice(&attested_header.state_root.0 .0),
				body_root: types::Hash256::from_slice(&attested_header.body_root.0 .0),
			}),
			sync_aggregate: eth_types::eth2::SyncAggregate {
				sync_committee_bits: SyncCommitteeBits(sync_committee_bits),
				sync_committee_signature: serde_json::from_str::<SignatureBytes>(
					&serde_json::to_string(&sync_aggregate.sync_committee_signature)?,
				)?,
			},
			signature_slot,
			finality_update: Self::get_finality_update(
				&types::BeaconBlockHeader {
					slot: Slot::new(finality_header.slot),
					proposer_index: finality_header.proposer_index,
					parent_root: types::Hash256::from_slice(&finality_header.parent_root.0 .0),
					state_root: types::Hash256::from_slice(&finality_header.state_root.0 .0),
					body_root: types::Hash256::from_slice(&finality_header.body_root.0 .0),
				},
				&beacon_state,
				&finalized_block_body,
			)?,
			sync_committee_update: match finality_beacon_state {
				None => None,
				Some(beacon_state) => Some(Self::get_next_sync_committee(&beacon_state)?),
			},
		})
	}

	fn get_next_sync_committee(
		beacon_state: &BeaconState<MainnetEthSpec>,
	) -> Result<SyncCommitteeUpdate, crate::Error> {
		let next_sync_committee =
			beacon_state.next_sync_committee().map_err(|_| MissNextSyncCommittee)?;

		let beacon_state_merkle_tree = BeaconStateMerkleTree::new(beacon_state);

		const BEACON_STATE_MERKLE_TREE_DEPTH: usize = 5;
		const BEACON_STATE_NEXT_SYNC_COMMITTEE_INDEX: usize = 23;

		let proof = beacon_state_merkle_tree
			.0
			.generate_proof(BEACON_STATE_NEXT_SYNC_COMMITTEE_INDEX, BEACON_STATE_MERKLE_TREE_DEPTH);

		let next_sync_committee_branch = proof.1;

		let next_sync_committee_branch = next_sync_committee_branch
			.into_iter()
			.map(|x| eth_types::H256::from(x.0.to_vec()))
			.collect();

		let sync_committee = SyncCommittee {
			pubkeys: eth_types::eth2::SyncCommitteePublicKeys(
				next_sync_committee
					.pubkeys
					.iter()
					.map(|x| eth_types::eth2::PublicKeyBytes(x.serialize()))
					.collect(),
			),
			aggregate_pubkey: eth_types::eth2::PublicKeyBytes(
				next_sync_committee.aggregate_pubkey.serialize(),
			),
		};

		Ok(SyncCommitteeUpdate { next_sync_committee: sync_committee, next_sync_committee_branch })
	}

	fn from_lighthouse_beacon_header(
		beacon_header: &BeaconBlockHeader,
	) -> eth_types::eth2::BeaconBlockHeader {
		eth_types::eth2::BeaconBlockHeader {
			slot: beacon_header.slot.as_u64(),
			proposer_index: beacon_header.proposer_index,
			parent_root: eth_types::H256::from(beacon_header.parent_root.0),
			state_root: eth_types::H256::from(beacon_header.state_root.0),
			body_root: eth_types::H256::from(beacon_header.body_root.0),
		}
	}

	fn get_sync_committee_bits(
		sync_committee_signature: &types::SyncAggregate<MainnetEthSpec>,
	) -> Result<[u8; 64], crate::Error> {
		match sync_committee_signature.clone().sync_committee_bits.as_ssz_bytes().try_into() {
			Ok(ba) => Ok(ba),
			Err(_) => Err(ErrorOnUnwrapSignatureBit.into()),
		}
	}

	fn get_finality_branch(
		beacon_state: &BeaconState<MainnetEthSpec>,
	) -> Result<Vec<H256>, crate::Error> {
		const BEACON_STATE_MERKLE_TREE_DEPTH: usize = 5;
		const BEACON_STATE_FINALIZED_CHECKPOINT_INDEX: usize = 20;

		let beacon_state_merkle_tree = BeaconStateMerkleTree::new(beacon_state);
		let mut proof = beacon_state_merkle_tree.0.generate_proof(
			BEACON_STATE_FINALIZED_CHECKPOINT_INDEX,
			BEACON_STATE_MERKLE_TREE_DEPTH,
		);

		let mut finality_branch = vec![beacon_state.finalized_checkpoint().epoch.tree_hash_root()];
		finality_branch.append(&mut proof.1);

		Ok(finality_branch
			.into_iter()
			.map(|x| eth_types::H256::from(x.0.to_vec()))
			.collect())
	}

	fn get_finality_update(
		finality_header: &BeaconBlockHeader,
		beacon_state: &BeaconState<MainnetEthSpec>,
		finalized_block_body: &BeaconBlockBody<MainnetEthSpec>,
	) -> Result<FinalizedHeaderUpdate, crate::Error> {
		let finality_branch = Self::get_finality_branch(beacon_state)?;
		let finalized_block_eth1data_proof =
			ExecutionBlockProof::construct_from_beacon_block_body(finalized_block_body)?;

		Ok(FinalizedHeaderUpdate {
			header_update: HeaderUpdate {
				beacon_header: Self::from_lighthouse_beacon_header(finality_header),
				execution_block_hash: eth_types::H256::from(
					finalized_block_eth1data_proof.get_execution_block_hash().0.to_vec(),
				),
				execution_hash_branch: finalized_block_eth1data_proof
					.get_proof()
					.iter()
					.map(|x| eth_types::H256::from(x.0.to_vec()))
					.collect(),
			},
			finality_branch,
		})
	}
}

#[cfg(test)]
mod tests {
	use crate::{
		beacon_rpc_client::BeaconRPCClient, config_for_tests::ConfigForTests,
		hand_made_finality_light_client_update::HandMadeFinalityLightClientUpdate,
	};
	use eth_types::eth2::LightClientUpdate;

	const TIMEOUT_SECONDS: u64 = 30;
	const TIMEOUT_STATE_SECONDS: u64 = 1000;

	fn get_test_config() -> ConfigForTests {
		ConfigForTests::load_from_toml("config_for_tests.toml".try_into().unwrap())
	}

	fn cmp_light_client_updates(
		hand_made_light_client_update: &LightClientUpdate,
		light_client_update: &LightClientUpdate,
	) {
		assert_eq!(
			serde_json::to_string(&hand_made_light_client_update.signature_slot).unwrap(),
			serde_json::to_string(&light_client_update.signature_slot).unwrap()
		);
		assert_eq!(
			serde_json::to_string(&hand_made_light_client_update.attested_beacon_header).unwrap(),
			serde_json::to_string(&light_client_update.attested_beacon_header).unwrap()
		);
		assert_eq!(
			serde_json::to_string(&hand_made_light_client_update.finality_update).unwrap(),
			serde_json::to_string(&light_client_update.finality_update).unwrap()
		);
		assert_eq!(
			serde_json::to_string(&hand_made_light_client_update.sync_aggregate).unwrap(),
			serde_json::to_string(&light_client_update.sync_aggregate).unwrap()
		);
	}

	#[ignore]
	#[tokio::test]
	async fn test_hand_made_finality_light_client_update() {
		let config = get_test_config();
		let beacon_rpc_client = BeaconRPCClient::new(
			&config.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);

		let light_client_period = BeaconRPCClient::get_period_for_slot(config.first_slot);

		let light_client_update =
			beacon_rpc_client.get_light_client_update(light_client_period).await.unwrap();

		let attested_slot = light_client_update.attested_beacon_header.slot;

		let hand_made_light_client_update =
			HandMadeFinalityLightClientUpdate::get_finality_light_client_update(
				&beacon_rpc_client,
				attested_slot,
				true,
			)
			.await
			.unwrap();

		cmp_light_client_updates(&hand_made_light_client_update, &light_client_update);

		assert_eq!(
			serde_json::to_string(&hand_made_light_client_update.sync_committee_update).unwrap(),
			serde_json::to_string(&light_client_update.sync_committee_update).unwrap()
		)
	}

	#[tokio::test]
	#[ignore]
	async fn test_hand_made_finality_light_client_update_from_file() {
		let config = get_test_config();
		let beacon_rpc_client = BeaconRPCClient::new(
			&config.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		let hand_made_light_client_update =
			HandMadeFinalityLightClientUpdate::get_finality_light_client_update_from_file(
				&beacon_rpc_client,
				&config.path_to_attested_state_for_period,
			)
			.await
			.unwrap();

		let light_client_period =
			BeaconRPCClient::get_period_for_slot(hand_made_light_client_update.signature_slot);

		let light_client_update =
			beacon_rpc_client.get_light_client_update(light_client_period).await.unwrap();

		cmp_light_client_updates(&hand_made_light_client_update, &light_client_update);
	}

	#[tokio::test]
	#[ignore]
	async fn test_hand_made_finality_light_client_update_from_file_with_next_sync_committee() {
		let config = get_test_config();
		let beacon_rpc_client = BeaconRPCClient::new(
			&config.beacon_endpoint,
			TIMEOUT_SECONDS,
			TIMEOUT_STATE_SECONDS,
			None,
		);
		let hand_made_light_client_update =
            HandMadeFinalityLightClientUpdate::get_light_client_update_from_file_with_next_sync_committee(
                &beacon_rpc_client,
                &config.path_to_attested_state_for_period,
                &config.path_to_finality_state_for_period,
            ).await.unwrap();

		let light_client_period =
			BeaconRPCClient::get_period_for_slot(hand_made_light_client_update.signature_slot);

		let light_client_update =
			beacon_rpc_client.get_light_client_update(light_client_period).await.unwrap();

		cmp_light_client_updates(&hand_made_light_client_update, &light_client_update);

		assert_eq!(
			serde_json::to_string(&hand_made_light_client_update.sync_committee_update).unwrap(),
			serde_json::to_string(&light_client_update.sync_committee_update).unwrap()
		)
	}
}
