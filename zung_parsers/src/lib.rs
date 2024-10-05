pub mod bencode;

use crate::bencode::Bencode;
use clap::{Args, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Custom parsers for various data formats
#[derive(Debug, Args)]
#[command(flatten_help = true, subcommand_required = true)]
pub struct ParsersArgs {
    #[command(subcommand)]
    command: ParsersCommands,
}

#[derive(Clone, Subcommand, Debug)]
#[command(arg_required_else_help = true)]
enum ParsersCommands {
    /// A parser for Bencode
    Bencode {
        #[command(subcommand)]
        command: BencodeCommands,
    },
}

#[derive(Clone, Subcommand, Debug)]
#[command(arg_required_else_help = true)]
enum BencodeCommands {
    /// Decode the bencode into a given format
    Decode {
        #[arg(value_enum)]
        /// Decode in the provided format.       
        format: Format,

        /// The file containing bencode data.
        file: PathBuf,
    },

    /// Try decoding a String of bencode for testing purposes.
    Try {
        /// Pass a bencode string directly as an argument
        input: String,
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

impl ParsersArgs {
    pub fn run(self) -> anyhow::Result<()> {
        // Run the commands
        match self.command {
            ParsersCommands::Bencode { command } => match command {
                BencodeCommands::Decode { file, format } => {
                    let file = std::fs::read(file)?;
                    let bencode = Bencode::from(file);

                    match format {
                        Format::Json => println!("{}", bencode.to_json_pretty()?),
                        Format::Yaml => println!("{}", bencode.to_yaml_string()?),
                        Format::Toml => println!("{}", bencode.to_toml_string()?),
                    }
                }

                BencodeCommands::Try { input } => {
                    let input = input.as_bytes();
                    let bencode = Bencode::from(input);
                    println!("{bencode}");
                }
            },
        }
        Ok(())
    }
}
