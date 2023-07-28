use std::path::PathBuf;

/// Cli tool to interact with Webb Relayer CLI
#[derive(Debug, Clone, clap::Parser)]
#[clap(next_help_heading = "Webb Relayer")]
pub struct RelayerCmd {
	/// Directory that contains configration files for the relayer.
	#[arg(long, value_name = "PATH")]
	pub relayer_config_dir: Option<PathBuf>,

	/// Light client relayer configuration directory.
	#[arg(long, value_name = "PATH")]
	pub light_client_relay_config_path: Option<PathBuf>,

	/// Light client init pallet configuration directory.
	#[arg(long, value_name = "PATH")]
	pub light_client_init_pallet_config_path: Option<PathBuf>,
}
