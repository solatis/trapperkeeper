//! Entry point of the `trapperkeeper` binary: parse the command line, resolve
//! the settings, run the subcommand.

mod cli;
mod config;
mod http;
mod logging;

use std::process::ExitCode;

use clap::Parser;

use cli::{Cli, Command};
use config::Settings;
use tracing_subscriber::filter::LevelFilter;

/// Runs the subcommand named on the command line and returns its exit status.
/// The tokio runtime owns the process: everything below is async wherever an
/// async form exists.
#[tokio::main]
async fn main() -> ExitCode {
    // Four stages, each with its own way of failing, and the order is the
    // rule: the settings are resolved before logging starts and before any
    // subcommand runs, so a bad configuration fails the same way for every
    // subcommand.
    //
    // 1. parse the command line;
    // 2. resolve the settings;
    // 3. install logging;
    // 4. dispatch on the subcommand.

    // 1. Parse the command line. clap prints help, the version or a usage
    //    error itself and exits with status 2, so no error reaches this
    //    function.
    let cli = Cli::parse();

    // 2. Resolve the settings. An error is printed and fails the process
    //    before any subcommand runs.
    let settings = match config::load(cli.config.as_deref(), &cli.overrides) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    // 3. Install logging. An error is printed with eprintln and fails the
    //    process, because no subscriber exists yet to log it.
    if let Err(error) = logging::init(LevelFilter::INFO) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }

    // 4. Dispatch on the subcommand.
    match cli.command {
        Command::Serve => serve(&settings).await,
        Command::Migrate => migrate(&settings),
    }
}

/// Runs the server and turns its I/O error into an exit status, so the http
/// module reports errors and never exits the process itself.
async fn serve(settings: &Settings) -> ExitCode {
    match http::serve(settings).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// Placeholder for the migration runner: reports that it is not implemented
/// and fails. The store issue replaces the body.
fn migrate(_settings: &Settings) -> ExitCode {
    // FAILURE rather than clap's 2, so a caller can tell a placeholder from a
    // usage error.
    eprintln!("migrate is not implemented yet");
    ExitCode::FAILURE
}
