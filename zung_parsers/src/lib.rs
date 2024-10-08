pub mod bencode;

use crate::bencode::*;
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Args)]
#[command(flatten_help = true, subcommand_required = true)]
pub struct BencodeArgs {
    #[command(subcommand)]
    command: BencodeCommands,
}

#[derive(Clone, Subcommand, Debug)]
#[command(arg_required_else_help = true)]
enum BencodeCommands {
    /// Decode the bencode into a given format
    Decode {
        /// The file containing bencode data.
        file: PathBuf,
    },

    /// Try decoding a String of bencode for testing purposes.
    Try {
        /// Pass a bencode string directly as an argument
        input: String,
    },
}

impl BencodeArgs {
    pub fn run_bencode(self) -> anyhow::Result<()> {
        // Run the commands
        match self.command {
            BencodeCommands::Decode { file } => {
                let file = std::fs::read(file)?;
                let bencode = Bencode::from_bytes(&file)?;

                println!("{bencode:#?}");
            }

            BencodeCommands::Try { input } => {
                let bencode = Bencode::from_string(&input)?;
                println!("{bencode:?}");
            }
        }
        Ok(())
    }
}
