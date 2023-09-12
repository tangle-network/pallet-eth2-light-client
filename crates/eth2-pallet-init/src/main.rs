use clap::Parser;
use std::string::String;
use subxt::{OnlineClient, PolkadotConfig};
use webb_eth2_pallet_init::{
	config::Config, init_pallet::init_pallet, substrate_pallet_client::EthClientPallet,
};
use webb_proposals::TypedChainId;

#[derive(Parser, Default, Debug)]
#[clap(version, about = "ETH2 contract initialization")]
struct Arguments {
	#[clap(short, long)]
	/// Path to config file
	config: String,

	#[clap(long, default_value_t = String::from("info"))]
	/// Log level (trace, debug, info, warn, error)
	log_level: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let args = Arguments::parse();
	let config =
		Config::load_from_toml(args.config.clone().try_into().expect("Incorrect config path"));

	let api = OnlineClient::<PolkadotConfig>::from_url(config.substrate_endpoint.clone())
		.await
		.expect("failed to connect to substrate node");

	let mut eth_client_contract = EthClientPallet::new(api, TypedChainId::None);
	init_pallet(&config, &mut eth_client_contract)
		.await
		.expect("Error on pallet initialization");

	Ok(())
}
