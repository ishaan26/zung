//! # Introduction
//!
//! Mini rust projects that target specific features of rust

pub mod progbar;

use clap::{Args, Subcommand};
use progbar::ProgBarExt;

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
            MiniCommands::Progbar { iter_count } => {
                use std::thread::sleep;
                use std::time::Duration;

                // test run UnBounded
                for _ in (0..iter_count).progbar() {
                    sleep(Duration::from_millis(50))
                }

                // test run Bounded
                for _ in (0..iter_count)
                    .progbar()
                    .bar_style('=')
                    .with_bounds('(', ')')
                {
                    sleep(Duration::from_millis(50))
                }
            }
        }
    }
}
