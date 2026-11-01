//! The command line of the `trapperkeeper` binary: the subcommands and the
//! options that stand before them.

use clap::{Parser, Subcommand};

/// The parsed command line.
#[derive(Parser)]
#[command(name = "trapperkeeper", version, about = "The TrapperKeeper server")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// The subcommands.
#[derive(Subcommand)]
pub enum Command {
    /// Run the server.
    Serve,
    /// Apply pending database migrations.
    Migrate,
}
