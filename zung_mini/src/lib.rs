//! # Introduction
//!
//! Mini rust projects that target specific features of rust

pub mod progbar;
pub mod strsplit;

use clap::{Args, Subcommand};
use progbar::ProgBarExt;
use strsplit::StrsplitExt;

/// An example Clap Argument builder. Install the [`zung`](https://crates.io/crates/zung) crate and
/// run `zung mini progbar` to see what options are available
#[derive(Debug, Args)]
#[command(flatten_help = true, subcommand_required = true)]
pub struct MiniArgs {
    #[command(subcommand)]
    command: MiniCommands,
}

#[derive(Clone, Subcommand, Debug)]
#[command(arg_required_else_help = true)]
enum MiniCommands {
    /// Print a progress bar to an iterator.
    Progbar {
        #[command(subcommand)]
        command: ProgBarCommands,
    },

    /// Perform splitting functions over a string.
    Strsplit {
        #[command(subcommand)]
        command: StrsplitCommands,
    },
}

#[derive(Clone, Subcommand, Debug)]
#[command(arg_required_else_help = true)]
enum StrsplitCommands {
    /// Split the provided string on the provided needle.
    Split {
        /// The needle to be fond in the haystack.
        #[arg(short, long)]
        needle: String,

        /// The haystack to find the needle in.
        #[arg(short, long)]
        string: String,
    },

    /// Split the provided string until the needle occurs in the String.
    Until {
        /// The needle to be fond in the haystack.
        #[arg(short, long)]
        needle: String,

        /// The haystack to find the needle in.
        #[arg(short, long)]
        string: String,
    },
}

#[derive(Clone, Subcommand, Debug)]
#[command(arg_required_else_help = true)]
enum ProgBarCommands {
    /// Runs the progbar on a simulated infinite loop.
    UnBounded {
        /// Custom message to display along with the spinner.
        #[arg(short, long, default_value_t = String::from("Simulating Loading..."))]
        message: String,
    },

    /// Runs the progbar on a simulated loop having len of iter_count.
    Bounded {
        /// Custom starting delimiter for the loading bar.
        #[arg(long, default_value_t = String::from("["))]
        delim_start: String,

        /// Custom ending delimiter for the loading bar.
        #[arg( long, default_value_t = String::from("]"))]
        delim_close: String,

        /// Custom bar style. Each value specified will be repeated on each iteration.
        #[arg(long, default_value_t = String::from("#"))]
        bar_style: String,

        /// Set custom length of the loop.
        #[arg(short, long, default_value_t = 50)]
        iter_count: u8,
    },
}

impl MiniArgs {
    pub fn run(self) {
        match self.command {
            MiniCommands::Progbar { command } => {
                use std::thread::sleep;
                use std::time::Duration;

                match command {
                    ProgBarCommands::UnBounded { message } => {
                        // test run UnBounded
                        for _ in (0..).progbar().with_message(&message) {
                            sleep(Duration::from_millis(50))
                        }
                    }
                    ProgBarCommands::Bounded {
                        delim_start,
                        delim_close,
                        bar_style,
                        iter_count,
                    } => {
                        // test run Bounded
                        for _ in (0..iter_count)
                            .progbar()
                            .with_bounds(delim_start, delim_close)
                            .bar_style(bar_style)
                        {
                            sleep(Duration::from_millis(50))
                        }
                    }
                }
            }

            MiniCommands::Strsplit { command } => match command {
                StrsplitCommands::Split { needle, string } => {
                    let result = string.strsplit(&needle).into_vec();
                    println!("{:?}", result);
                }
                StrsplitCommands::Until { needle, string } => {
                    let result = string.strsplit(needle).till_needle();
                    println!("{:?}", result);
                }
            },
        }
    }
}
