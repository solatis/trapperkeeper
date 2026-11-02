//! The command line of the `trapperkeeper` binary: the subcommands, the
//! options that stand before or after them, and the settings a flag may
//! override. The overrides are the top layer of `config::load`, so
//! `Overrides` serializes to the same tree as `config::Settings`.

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use serde::Serialize;

use crate::config::LogLevel;

/// The parsed command line.
#[derive(Parser)]
#[command(name = "trapperkeeper", version, about = "The TrapperKeeper server")]
pub struct Cli {
    /// Path of the TOML configuration file. No file is read when absent, so a
    /// stray file in the working directory changes nothing.
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    #[command(flatten)]
    pub overrides: Overrides,

    #[command(subcommand)]
    pub command: Command,
}

/// Settings a flag may override. Every field is optional, and an absent flag
/// leaves the value of the layers below in place. Nested settings are nested
/// here too, so the tree matches. `Default` is the "no flags" value the tests
/// pass.
#[derive(Args, Serialize, Default)]
pub struct Overrides {
    /// Address the server listens on, `host:port`.
    #[arg(long, global = true, value_name = "ADDR")]
    // An absent flag must serialize to nothing: serialized as a null, it would
    // be taken by the merge over the layer below, and extraction would then
    // fail with a type error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen: Option<SocketAddr>,

    #[command(flatten)]
    pub log: LogOverrides,

    #[command(flatten)]
    pub database: DatabaseOverrides,
}

/// Overrides of the `log` table. With no flag set it serializes to an empty
/// table, which the merge unions into the layer below without changing it.
#[derive(Args, Serialize, Default)]
pub struct LogOverrides {
    /// Lowest log level written: trace, debug, info, warn, error or off.
    #[arg(long = "log-level", global = true, value_name = "LEVEL")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<LogLevel>,
}

/// Overrides of the `database` table. With no flag set it serializes to an
/// empty table, as `LogOverrides` does.
#[derive(Args, Serialize, Default)]
pub struct DatabaseOverrides {
    /// Where the store lives, as a URL: `sqlite://<path>`.
    #[arg(long = "database-url", global = true, value_name = "URL")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// The subcommands. Each one resolves the settings first and then runs.
#[derive(Subcommand)]
pub enum Command {
    /// Run the server.
    Serve,
    /// Apply pending database migrations.
    Migrate,
}
