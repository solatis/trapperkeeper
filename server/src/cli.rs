//! The command line of the `trapperkeeper` binary: the subcommands, the
//! options that stand before or after them, and the settings a flag may
//! override. The overrides are the top layer of `config::load`, so
//! `Overrides` serializes to the same tree as `config::Settings`.

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use serde::Serialize;

/// The parsed command line.
#[derive(Parser)]
#[command(name = "trapperkeeper", version, about = "The TrapperKeeper server")]
pub struct Cli {
    /// Path of the TOML configuration file. No file is read when absent.
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    #[command(flatten)]
    pub overrides: Overrides,

    #[command(subcommand)]
    pub command: Command,
}

/// Settings a flag may override. Every field is optional, and an absent flag
/// leaves the value of the layers below in place. `Default` is the "no flags"
/// value the tests pass.
#[derive(Args, Serialize, Default)]
pub struct Overrides {
    /// Address the server listens on, `host:port`.
    #[arg(long, global = true, value_name = "ADDR")]
    // An absent flag must serialize to nothing: serialized as a null, it would
    // be taken by the merge over the layer below, and extraction would then
    // fail with a type error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen: Option<SocketAddr>,
}

/// The subcommands. Each one resolves the settings first and then runs.
#[derive(Subcommand)]
pub enum Command {
    /// Run the server.
    Serve,
    /// Apply pending database migrations.
    Migrate,
}
