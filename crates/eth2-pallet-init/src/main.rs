use clap::Parser;
use std::string::String;
use webb_eth2_pallet_init::{
	config::Config,
	init_pallet::init_pallet,
	substrate_pallet_client::{setup_api, EthClientPallet},
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

// fn init_log(args: &Arguments, config: &Config) {
// 	let log_level_filter = match args.log_level.as_str() {
// 		"trace" => LevelFilter::Trace,
// 		"debug" => LevelFilter::Debug,
// 		"warn" => LevelFilter::Warn,
// 		"error" => LevelFilter::Error,
// 		_ => LevelFilter::Info,
// 	};

// 	let mut path_to_log_file = "./eth2-pallet-init.log".to_string();
// 	if let Some(out_dir) = config.clone().output_dir {
// 		path_to_log_file = out_dir.clone() + "/" + "eth2-pallet-init.log";
// 		std::fs::create_dir_all(out_dir).expect("Error during creation output dirs in path");
// 	}

// 	log::set_boxed_logger(Box::new(SimpleLogger::new(path_to_log_file)))
// 		.map(|()| log::set_max_level(log_level_filter))
// 		.expect("Error during logger creation");
// }

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let args = Arguments::parse();
	let config =
		Config::load_from_toml(args.config.clone().try_into().expect("Incorrect config path"));
	// init_log(&args, &config);

	let api = setup_api().await.unwrap();
	let mut eth_client_contract = EthClientPallet::new(api, TypedChainId::None);
	init_pallet(&config, &mut eth_client_contract)
		.await
		.expect("Error on contract initialization");

	Ok(())
}
