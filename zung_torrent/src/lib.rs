#![doc = include_str!("../README.md")]

pub mod meta_info;
use meta_info::MetaInfo;

#[cfg(feature = "client")]
mod client;
pub use client::Client;

use clap::{Args, Subcommand, ValueEnum};
use meta_info::SortOrd;
use std::path::PathBuf;

/// Interact with torrent on the commandline. Install the [`zung`](https://crates.io/crates/zung)
/// crate and run `zung torrent --help` to see what options are available
#[derive(Debug, Args)]
#[command(flatten_help = true, subcommand_required = true)]
pub struct TorrentArgs {
    #[command(subcommand)]
    command: TorrentCommands,
}

#[derive(Clone, Subcommand, Debug)]
#[command(arg_required_else_help = true)]
enum TorrentCommands {
    /// Prints the information contained in the torrent file. The information is produced fully
    /// locally without sending any internet requests.
    Info {
        /// Torrent File to process
        #[arg(short, long, required = true)]
        file: PathBuf,

        /// Print the files contained in the torrent along with the general info.
        #[arg(long, required = false)]
        with_files: bool,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Format {
    /// Convert to json format
    Json,

    /// Convert to yaml format
    Yaml,

    /// Convert to toml format
    Toml,
}

impl TorrentArgs {
    pub fn run(self) -> anyhow::Result<()> {
        // Run the commands
        match self.command {
            TorrentCommands::Info { file, with_files } => {
                let torrent = Client::new(&file)?;
                torrent.print_torrent_info();
                if with_files {
                    torrent.print_files_by_size(SortOrd::Ascending);
                }
            }
        }

        Ok(())
    }
}
