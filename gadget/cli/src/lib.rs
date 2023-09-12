use std::path::PathBuf;

/// Cli tool to interact with Webb Light Client Relayer CLI
#[derive(Debug, Clone, clap::Parser)]
#[clap(next_help_heading = "Webb Light Client Relayer")]
pub struct LightClientRelayerCmd {
	/// Light client relayer configuration directory.
	#[arg(long, value_name = "PATH")]
	pub light_client_relay_config_path: Option<PathBuf>,

	/// Light client init pallet configuration directory.
	#[arg(long, value_name = "PATH")]
	pub light_client_init_pallet_config_path: Option<PathBuf>,
}
