#![doc = include_str!("../README.md")]

mod meta_info;

#[cfg(feature = "client")]
mod client;
pub use client::Client;

pub use meta_info::MetaInfo;

use clap::{Args, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Args)]
#[command(flatten_help = true, subcommand_required = true)]
pub struct TorrentArgs {
    #[command(subcommand)]
    command: TorrentCommands,
}

#[derive(Clone, Subcommand, Debug)]
enum TorrentCommands {
    /// Prints the information contained in the torrent file
    Info {
        /// The Bencode file to decode
        #[arg(short, long, required = true)]
        file: PathBuf,
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
            TorrentCommands::Info { file } => {
                let torrent = Client::new(&file)?;
                torrent.print_torrent_info();
            }
        }

        Ok(())
    }
}
