//! # Introduction
//!
//! Mini rust projects that target specific features of rust

/// Implementation of a progress bar. WIP rn.
pub mod progbar;

use clap::{Args, Subcommand};

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
        #[arg(short, long, default_value_t = 50)]
        iter_count: u8,
    },
}

impl MiniArgs {
    pub fn run(self) {
        match self.command {
            MiniCommands::Progbar { iter_count } => progbar::run_progbar(iter_count),
        }
    }
}
