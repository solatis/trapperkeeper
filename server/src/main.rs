//! Entry point of the `trapperkeeper` binary: parse the command line, resolve
//! the settings, run the subcommand.

mod cli;
mod config;
mod http;
mod logging;
mod store;

use std::process::ExitCode;

use clap::Parser;

use cli::{Cli, Command};
use config::Settings;

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

    // 2. Resolve the settings. An error is printed with eprintln and fails the
    //    process before any subcommand runs: the level is a setting, so
    //    logging cannot start before the settings are known.
    let settings = match config::load(cli.config.as_deref(), &cli.overrides) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    // 3. Install logging at the level the settings resolved to. An error is
    //    printed with eprintln and fails the process, because no subscriber
    //    exists yet to log it. The resolved settings are then logged at debug
    //    as one JSON string, so a support case sees what the server believed
    //    without reading three sources, and the settings test reads them back
    //    without parsing Debug text. A struct of plain values always
    //    serializes.
    if let Err(error) = logging::init(settings.log.level.0) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    let json = serde_json::to_string(&settings).expect("plain values serialize");
    tracing::debug!(settings = %json, "settings resolved");

    // 4. Dispatch on the subcommand.
    match cli.command {
        Command::Serve => serve(&settings).await,
        Command::Migrate => migrate(&settings).await,
    }
}

/// Checks the database against the binary, serves until a signal, and
/// returns the exit status.
async fn serve(settings: &Settings) -> ExitCode {
    // 1. open the database without creating it
    // 2. check it against the binary's migrations
    // 3. serve
    //
    // Both refusals come before the listener binds, so a refused server is
    // never reachable.

    // 1. Only migrate creates the database, so a mistyped path fails here
    //    instead of serving an empty database.
    let store = match store::open(&settings.database.url).await {
        Ok(store) => store,
        Err(error) => {
            eprintln!(
                "{error}\nthe database does not open; when it does not exist, `trapperkeeper migrate` creates it"
            );
            return ExitCode::FAILURE;
        }
    };

    match store::migrate::check(&store).await {
        Ok(None) => {}
        Ok(Some(mismatch)) => {
            eprintln!("{mismatch}");
            return ExitCode::FAILURE;
        }
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    }

    match http::serve(settings).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// Creates the database when it is missing, applies the pending migrations,
/// and returns the exit status.
async fn migrate(settings: &Settings) -> ExitCode {
    let store = match store::create(&settings.database.url).await {
        Ok(store) => store,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    match store::migrate::apply(&store).await {
        Ok(count) => {
            // At info, so an operator sees what migrate did.
            tracing::info!(count, "migrations applied");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
