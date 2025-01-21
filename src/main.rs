use clap::{Parser, Subcommand};

use zung_mini::MiniArgs;
use zung_parsers::ParserArgs;
use zung_torrent::TorrentArgs;

#[derive(Parser)]
#[command(author, version, about, long_about = None, styles=get_styles())] // Read from `Cargo.toml`
struct Cli {
    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Mini projects implemented in rust
    Mini(MiniArgs),

    /// Parsers for different data formats
    Parsers(ParserArgs),

    /// Torrent Client
    Torrent(TorrentArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    set_subscribers();

    let cli = Cli::parse();

    match cli.commands {
        Commands::Mini(mini_args) => mini_args.run(),
        Commands::Parsers(bencode_args) => bencode_args.run()?,
        Commands::Torrent(torrent_args) => {
            torrent_args.run().await?;
        }
    }

    Ok(())
}

fn get_styles() -> clap::builder::Styles {
    clap::builder::Styles::styled()
        .usage(
            anstyle::Style::new()
                .bold()
                .underline()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Blue))),
        )
        .header(
            anstyle::Style::new()
                .bold()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Blue))),
        )
        .literal(
            anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Green))),
        )
        .invalid(
            anstyle::Style::new()
                .bold()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Red))),
        )
        .error(
            anstyle::Style::new()
                .bold()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Red))),
        )
        .valid(
            anstyle::Style::new()
                .bold()
                .underline()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Cyan))),
        )
        .placeholder(
            anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Yellow))),
        )
}

fn set_subscribers() {
    // For tokio console.
    // {
    //     use tracing_subscriber::prelude::*;
    //
    //     // spawn the console server in the background,
    //     // returning a `Layer`:
    //     let console_layer = console_subscriber::spawn();
    //
    //     // build a `Subscriber` by combining layers with a
    //     // `tracing_subscriber::Registry`:
    //     tracing_subscriber::registry()
    //         .with(console_layer)
    //         .with(
    //             tracing_subscriber::fmt::layer()
    //                 .with_level(true)
    //                 .without_time(),
    //         )
    //         .init();
    // }

    // No need for tokio console in release builds.
    // #[cfg(not(debug_assertions))]
    {
        tracing_subscriber::fmt()
            // all spans/events with a level higher than TRACE (e.g, info, warn, etc.)
            // will be written to stdout.
            .with_env_filter("zung=info")
            .without_time()
            .compact()
            // display source code file paths
            .with_file(false)
            // display source code line numbers
            .with_line_number(false)
            // disable targets
            .with_target(false)
            .init(); // sets this to be the default, global subscriber for this application.
    }
}

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert()
}
