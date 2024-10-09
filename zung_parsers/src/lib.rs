pub mod bencode;

use crate::bencode::*;
use clap::{Args, Subcommand, ValueEnum};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
};

#[derive(Debug, Args)]
#[command(flatten_help = true, subcommand_required = true)]
pub struct ParserArgs {
    #[command(subcommand)]
    command: BencodeArgs,
}

#[derive(Debug, Subcommand)]
#[command(flatten_help = true, subcommand_required = true)]
enum BencodeArgs {
    // A Bencode parser.
    Bencode {
        #[command(subcommand)]
        commands: BencodeCommands,
    },
}

#[derive(Clone, Subcommand, Debug)]
#[command(arg_required_else_help = true)]
enum BencodeCommands {
    /// Decode the bencode into a given format
    Decode {
        /// Decode in the provided format.       
        #[arg(long, value_enum)]
        format: Format,

        /// The Bencode file to decode
        #[arg(short, long)]
        file: PathBuf,

        /// Path to output the decoded data format in.
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Try decoding a String of bencode for testing purposes. This simply prints out the decoded
    /// data model.
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

impl ParserArgs {
    pub fn run(self) -> anyhow::Result<()> {
        // Run the commands
        match self.command {
            BencodeArgs::Bencode { commands } => match commands {
                BencodeCommands::Decode {
                    format,
                    file,
                    output,
                } => {
                    let file = std::fs::read(file)?;
                    let bencode = Bencode::from_bytes(&file)?;

                    let file = File::create(output)?;
                    let mut buf_writer = BufWriter::new(file);

                    match format {
                        Format::Json => serde_json::to_writer_pretty(buf_writer, &bencode)?,
                        Format::Yaml => serde_yaml::to_writer(buf_writer, &bencode)?,
                        Format::Toml => {
                            let b = toml::to_string_pretty(&bencode)?;
                            buf_writer.write_all(b.as_bytes())?;
                        }
                    };
                }

                BencodeCommands::Try { input } => {
                    let bencode = Bencode::from_string(&input)?;
                    println!("{bencode:?}");
                }
            },
        }
        Ok(())
    }
}
