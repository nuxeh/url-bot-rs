use crate::VERSION;
use std::path::PathBuf;
use structopt::{StructOpt, clap::arg_enum};

arg_enum!(
    #[derive(Debug, StructOpt)]
    pub enum ExportFormat {
        TOML,
        Json,
        Nix,
    }
);

#[derive(Debug, StructOpt)]
#[structopt(name = "url-bot-rs", about = "URL munching IRC bot.", version = VERSION.as_str())]
pub struct Args {
    /// Show extra information.
    #[structopt(short, long, parse(from_occurrences))]
    pub verbose: usize,

    /// Force timestamps.
    #[structopt(short, long)]
    pub timestamp: bool,

    /// Use configuration file(s) at <conf>.
    #[structopt(short, long, parse(from_os_str))]
    pub conf: Vec<PathBuf>,

    /// Search for configuration file(s) in <conf-dir>.
    #[structopt(short = "d", long, parse(from_os_str))]
    pub conf_dir: Vec<PathBuf>,

    /// Export all loaded configurations in various formats
    #[structopt(name = "export", long, case_insensitive = true)]
    pub export_format: Option<ExportFormat>,
}

