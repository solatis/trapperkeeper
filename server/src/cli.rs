//! The command line of the `trapperkeeper` binary: the subcommands and the
//! options that stand before or after them.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// The parsed command line.
#[derive(Parser)]
#[command(name = "trapperkeeper", version, about = "The TrapperKeeper server")]
pub struct Cli {
    /// Path of the TOML configuration file. No file is read when absent.
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

/// The subcommands. Each one resolves the settings first and then runs.
#[derive(Subcommand)]
pub enum Command {
    /// Run the server.
    Serve,
    /// Apply pending database migrations.
    Migrate,
}
