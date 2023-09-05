use bitvec::{order::Lsb0, prelude::BitVec};
use consensus_types::{
	compute_domain, compute_fork_version_by_slot, compute_signing_root, get_participant_pubkeys,
	DOMAIN_SYNC_COMMITTEE, MIN_SYNC_COMMITTEE_PARTICIPANTS,
};
use eth_types::{
	eth2::{BeaconBlockHeader, Epoch, ForkVersion, LightClientUpdate, SyncCommittee},
	H256,
};

use tree_hash::Hash256;

#[cfg(test)]
pub mod config_for_tests;

fn h256_to_hash256(hash: H256) -> Hash256 {
	Hash256::from_slice(hash.0.as_bytes())
}

fn tree_hash_h256_to_eth_type_h256(hash: tree_hash::Hash256) -> eth_types::H256 {
	eth_types::H256::from(hash.0.as_slice())
}

fn to_lighthouse_beacon_block_header(
	bridge_beacon_block_header: &BeaconBlockHeader,
) -> BeaconBlockHeader {
	BeaconBlockHeader {
		slot: bridge_beacon_block_header.slot,
		proposer_index: bridge_beacon_block_header.proposer_index,
		parent_root: eth_types::H256(h256_to_hash256(bridge_beacon_block_header.parent_root)),
		state_root: eth_types::H256(h256_to_hash256(bridge_beacon_block_header.state_root)),
		body_root: eth_types::H256(h256_to_hash256(bridge_beacon_block_header.body_root)),
	}
}

pub fn is_correct_finality_update(
	fork_epoch: Epoch,
	fork_version: ForkVersion,
	genesis_validators_root: [u8; 32],
	light_client_update: &LightClientUpdate,
	sync_committee: SyncCommittee,
) -> anyhow::Result<bool> {
	let sync_committee_bits =
		BitVec::<u8, Lsb0>::from_slice(&light_client_update.sync_aggregate.sync_committee_bits.0);

	let sync_committee_bits_sum: u64 = sync_committee_bits.count_ones().try_into()?;
	if sync_committee_bits_sum < MIN_SYNC_COMMITTEE_PARTICIPANTS {
		return Ok(false)
	}
	if sync_committee_bits_sum * 3 < (sync_committee_bits.len() * 2).try_into()? {
		return Ok(false)
	}

	let participant_pubkeys =
		get_participant_pubkeys(&sync_committee.pubkeys.0, &sync_committee_bits);
	let fork_version =
		compute_fork_version_by_slot(light_client_update.signature_slot, fork_epoch, fork_version)
			.expect("Unsupported fork");
	let domain =
		compute_domain(DOMAIN_SYNC_COMMITTEE, fork_version, genesis_validators_root.into());

	let attested_beacon_header_root = tree_hash::TreeHash::tree_hash_root(
		&to_lighthouse_beacon_block_header(&light_client_update.attested_beacon_header),
	);
	let signing_root =
		compute_signing_root(tree_hash_h256_to_eth_type_h256(attested_beacon_header_root), domain);

	let aggregate_signature = bls::AggregateSignature::deserialize(
		&light_client_update.sync_aggregate.sync_committee_signature.0,
	)?;
	let mut pubkeys: Vec<bls::PublicKey> = vec![];
	for pubkey in participant_pubkeys {
		pubkeys.push(bls::PublicKey::deserialize(&pubkey.0)?);
	}

	Ok(aggregate_signature.fast_aggregate_verify(
		h256_to_hash256(signing_root).0.into(),
		&pubkeys.iter().collect::<Vec<_>>(),
	))
}

#[cfg(test)]
mod tests {
	use std::str::FromStr;

	use crate::{config_for_tests::ConfigForTests, is_correct_finality_update};
	use consensus_types::network_config::{Network, NetworkConfig};
	use eth_types::eth2::{LightClientUpdate, SyncCommittee};

	fn get_config() -> ConfigForTests {
		ConfigForTests::load_from_toml("config_for_tests.toml".try_into().unwrap())
	}

	#[test]
	fn smoke_verify_finality_update() {
		let config = get_config();

		let light_client_updates: Vec<LightClientUpdate> = serde_json::from_str(
			&std::fs::read_to_string(config.path_to_light_client_updates)
				.expect("Unable to read file"),
		)
		.unwrap();

		let current_sync_committee: SyncCommittee = serde_json::from_str(
			&std::fs::read_to_string(config.path_to_current_sync_committee.clone())
				.expect("Unable to read file"),
		)
		.unwrap();
		let next_sync_committee: SyncCommittee = serde_json::from_str(
			&std::fs::read_to_string(config.path_to_next_sync_committee.clone())
				.expect("Unable to read file"),
		)
		.unwrap();

		let network: NetworkConfig =
			NetworkConfig::new(&Network::from_str(&config.network_name).unwrap());
		let fork_epoch = network.bellatrix_fork_epoch;
		let fork_version = network.bellatrix_fork_version;
		let genesis_validators_root = network.genesis_validators_root;

		assert!(is_correct_finality_update(
			fork_epoch,
			fork_version,
			genesis_validators_root,
			&light_client_updates[0],
			current_sync_committee
		)
		.unwrap());

		assert!(!is_correct_finality_update(
			fork_epoch,
			fork_version,
			genesis_validators_root,
			&light_client_updates[0],
			next_sync_committee
		)
		.unwrap());
	}
}
