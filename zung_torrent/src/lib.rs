#![doc = include_str!("../README.md")]
#![allow(clippy::mutable_key_type)] // The Socket addr is the hash key which is not mutable.

#[cfg(feature = "client")]
mod client;
pub mod meta_info;
pub mod sources;

pub use client::Client;
pub use client::PeerID;
use colored::Colorize;
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

        /// Print the download sources contained within the torrent file.
        #[arg(long, required = false)]
        with_sources: bool,
    },

    Test {
        /// Torrent File to process
        #[arg(short, long, required = true)]
        file: PathBuf,
    },
}

impl TorrentArgs {
    pub async fn run(self) -> anyhow::Result<()> {
        // Run the commands
        match self.command {
            TorrentCommands::Info {
                file,
                with_files,
                with_sources,
            } => {
                let torrent = Client::new(file)?;

                torrent.print_torrent_info();

                if with_files {
                    torrent.print_files_by_size(SortOrd::Ascending);
                }

                if with_sources {
                    torrent.print_download_sources();
                }
            }
            TorrentCommands::Test { file } => {
                let torrent = Client::new(file)?;

                let info_hash = torrent.info_hash().as_encoded();

                torrent.sources().announce_all(info_hash).await;

                if let Some(list) = torrent.sources().tracker_list() {
                    for t in list {
                        if t.is_connected() {
                            println!("{} -> {}", t.url().cyan(), t.trys())
                        }
                    }
                }

                torrent.sources().retry_connect_all(info_hash).await;

                if let Some(list) = torrent.sources().tracker_list() {
                    for t in list {
                        dbg!(t);
                    }
                }
            }
        }

        Ok(())
    }
}
