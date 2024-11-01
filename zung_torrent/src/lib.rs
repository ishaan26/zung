#![doc = include_str!("../README.md")]

#[cfg(feature = "client")]
mod client;
pub mod meta_info;
pub mod trackers;

pub use client::Client;
pub use client::PeerID;
use meta_info::MetaInfo;

use clap::{Args, Subcommand};
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
