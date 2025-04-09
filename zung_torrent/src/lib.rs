#![doc = include_str!("../README.md")]
#![allow(clippy::mutable_key_type)] // The Socket addr is the hash key which is not mutable.

#[cfg(feature = "client")]
mod client;
pub mod download;
pub mod http_seeders;
pub mod meta_info;
pub mod peers;
pub mod trackers;

pub use client::Client;
pub use client::PeerID;
use meta_info::MetaInfo;

use clap::{Args, Subcommand};
use meta_info::SortOrd;
use std::path::PathBuf;
use std::time::Duration;

pub const TIMEOUT_DURATION: Duration = Duration::from_secs(50);
pub const CONCURRENCY_LIMIT: usize = 150;

const URL_ENCODE_TABLE: [[u8; 3]; 256] = {
    let mut table = [[0; 3]; 256];
    let mut i: u16 = 0;
    while i <= 255 {
        let high = if (i >> 4) < 10 {
            b'0' + (i >> 4) as u8
        } else {
            b'A' + (i >> 4) as u8 - 10
        };
        let low = if (i & 0xF) < 10 {
            b'0' + (i & 0xF) as u8
        } else {
            b'A' + (i & 0xF) as u8 - 10
        };

        table[i as usize] = [b'%', high, low];
        i += 1;
    }
    table
};

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

    Download {
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
            TorrentCommands::Download { file } => {
                let torrent = Client::new(file)?;

                torrent.download_from_trackers().await?;
            }
        }

        Ok(())
    }
}
