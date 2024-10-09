pub mod bencode;

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
enum BencodeCommands {
    /// Decode the bencode into a given format
    Decode {
        /// Decode in the provided format.       
        #[arg(long, value_enum, required = true)]
        format: Format,

        /// The Bencode file to decode
        #[arg(short, long, required = true)]
        file: PathBuf,

        /// Path to output the decoded data format in.
        #[arg(short, long, required = true)]
        output: PathBuf,
    },

    /// Encode to bencode from given format
    Encode {
        /// Decode in the provided format.       
        #[arg(long, value_enum, required = true)]
        format: Format,

        /// File containing the format data
        #[arg(short, long, required = true)]
        file: PathBuf,

        /// Path to output the decoded data format in.
        #[arg(short, long, required = true)]
        output: PathBuf,
    },

    /// Try decoding a String of bencode for testing purposes. This simply prints out the decoded
    /// data model.
    Try {
        /// The Bencode file to decode
        #[arg(short, long)]
        file: Option<PathBuf>,
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
                    let bencode = bencode::parse(&file)?;

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

                BencodeCommands::Encode {
                    format,
                    file,
                    output,
                } => {
                    let file_read = std::fs::read(file)?;

                    let file_write = File::create(output)?;
                    let mut buf_writer = BufWriter::new(file_write);

                    match format {
                        Format::Json => {
                            let value: serde_json::Value = serde_json::from_slice(&file_read)?;
                            let bencode = bencode::to_string(&value)?;
                            write!(buf_writer, "{bencode}")?
                        }
                        Format::Yaml => {
                            let value: serde_yaml::Value = serde_yaml::from_slice(&file_read)?;
                            let bencode = bencode::to_string(&value)?;
                            write!(buf_writer, "{bencode}")?
                        }
                        Format::Toml => unimplemented!(),
                    };
                }

                BencodeCommands::Try { file } => {
                    if let Some(file) = file {
                        #[derive(
                            serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq,
                        )]
                        struct TorrentRepr {
                            announce: String,

                            #[serde(rename = "announce-list")]
                            announce_list: Option<Vec<Vec<String>>>,

                            comment: Option<String>,

                            #[serde(rename = "created by")]
                            created_by: Option<String>,

                            #[serde(rename = "creation date")]
                            creation_date: Option<usize>,
                        }

                        let file = std::fs::read(file)?;

                        let torrent: TorrentRepr = bencode::from_bytes(&file)?;
                        println!("{torrent:#?}")
                    }
                }
            },
        }
        Ok(())
    }
}
