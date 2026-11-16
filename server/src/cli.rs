//! The command line of the `trapperkeeper` binary: the subcommands, the
//! options that stand before or after them, and the settings a flag may
//! override.

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

/// Settings a flag may override. Each flag is an `Option`, and the tables
/// `log` and `database` are plain structs of such flags. An absent flag leaves
/// the value of the layers below in place. The tables nest as in
/// `config::Settings`, so figment merges each flag over the setting of the
/// same key. `Default` is the "no flags" value the tests pass. The struct has
/// no `auth` table, because `auth.signing_key` and `auth.previous_signing_key`
/// have no flag, and they have none because ADR-0013 warns on any secret given
/// as a flag (ADR-0025).
#[derive(Args, Serialize, Default)]
pub struct Overrides {
    /// Address the server listens on, an IP address and a port, such as
    /// `127.0.0.1:8080` or `[::1]:8080`.
    #[arg(long, global = true, value_name = "ADDR")]
    // An absent flag serializes to nothing. figment's merge takes the incoming
    // value whatever it is, so a null would replace the value of the lower
    // layers, and `extract` would then fail on the null.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen: Option<SocketAddr>,

    #[command(flatten)]
    pub log: LogOverrides,

    #[command(flatten)]
    pub database: DatabaseOverrides,
}

impl Overrides {
    /// The flags given on the command line that carry a secret, by name.
    #[must_use]
    pub fn secrets(&self) -> Vec<&'static str> {
        let mut secrets = Vec::new();
        // The database URL is the only secret setting that has a flag. The
        // signing keys, the other secret settings, have no flag.
        if self.database.url.is_some() {
            secrets.push("--database-url");
        }
        secrets
    }
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
    /// Where the store lives, as a URL: `sqlite://<path>`, `postgres://...` or
    /// `mariadb://...`.
    #[arg(long = "database-url", global = true, value_name = "URL")]
    #[serde(skip_serializing_if = "Option::is_none")]
    // The field is a `String` and not a `DatabaseUrl`, which serializes with
    // its password replaced by `***`. figment merges the serialized flags over
    // the lower layers, so the masked text would become the URL that opens.
    pub url: Option<String>,
}

/// The subcommands.
#[derive(Subcommand)]
pub enum Command {
    /// Run the server. Create the database when nothing exists at the URL, set
    /// up an empty database first, and refuse to start without
    /// `auth.signing_key`.
    Serve,
    /// Create the database when it is missing, apply the pending migrations,
    /// and create the superadmin when it is missing and log its one-time
    /// password.
    Migrate,
    /// Give the superadmin a new one-time password and print it, creating the
    /// superadmin when it is missing.
    ResetSuperadmin,
}
