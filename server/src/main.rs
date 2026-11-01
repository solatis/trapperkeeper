//! Entry point of the `trapperkeeper` binary: parse the command line, resolve
//! the settings, run the subcommand.

mod cli;
mod config;

use std::process::ExitCode;

use clap::Parser;

use cli::{Cli, Command};
use config::Settings;

/// Runs the subcommand named on the command line and returns its exit status.
fn main() -> ExitCode {
    // Three stages, each with its own way of failing, and the order is the
    // rule: the settings are resolved before any subcommand runs, so a bad
    // configuration fails the same way for every subcommand.
    //
    // 1. parse the command line;
    // 2. resolve the settings;
    // 3. dispatch on the subcommand.

    // 1. Parse the command line. clap prints help, the version or a usage
    //    error itself and exits with status 2, so no error reaches this
    //    function.
    let cli = Cli::parse();

    // 2. Resolve the settings. An error is printed and fails the process
    //    before any subcommand runs.
    let settings = match config::load() {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    // 3. Dispatch on the subcommand.
    match cli.command {
        Command::Serve => serve(&settings),
        Command::Migrate => migrate(&settings),
    }
}

/// Placeholder for the server: reports that it is not implemented and fails.
/// Issue #4 replaces the body.
fn serve(_settings: &Settings) -> ExitCode {
    not_implemented("serve")
}

/// Placeholder for the migration runner: reports that it is not implemented
/// and fails. The store issue replaces the body.
fn migrate(_settings: &Settings) -> ExitCode {
    not_implemented("migrate")
}

/// Reports on stderr that `subcommand` is not implemented yet and returns the
/// failure status.
fn not_implemented(subcommand: &str) -> ExitCode {
    // FAILURE rather than clap's 2, so a caller can tell a placeholder from a
    // usage error.
    eprintln!("{subcommand} is not implemented yet");
    ExitCode::FAILURE
}
