//! Entry point of the `trapperkeeper` binary: parse the command line and run
//! the subcommand.

mod cli;

use std::process::ExitCode;

use clap::Parser;

use cli::{Cli, Command};

/// Runs the subcommand named on the command line and returns its exit status.
fn main() -> ExitCode {
    // Two stages, each with its own way of failing:
    //
    // 1. parse the command line;
    // 2. dispatch on the subcommand.

    // 1. Parse the command line. clap prints help, the version or a usage
    //    error itself and exits with status 2, so no error reaches this
    //    function.
    let cli = Cli::parse();

    // 2. Dispatch on the subcommand.
    match cli.command {
        Command::Serve => serve(),
        Command::Migrate => migrate(),
    }
}

/// Placeholder for the server: reports that it is not implemented and fails.
/// Issue #4 replaces the body.
fn serve() -> ExitCode {
    not_implemented("serve")
}

/// Placeholder for the migration runner: reports that it is not implemented
/// and fails. The store issue replaces the body.
fn migrate() -> ExitCode {
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
