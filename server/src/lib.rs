//! The server behind the `trapperkeeper` binary, which calls `run`.

mod accounts;
mod audit;
mod auth;
mod cli;
mod config;
mod http;
mod logging;
mod names;
mod scope;
mod sessions;
pub mod setup;
mod store;
mod superadmin;
mod tenants;
mod users;

// The integration tests reach the module `setup` and the items re-exported
// below. No other item of the crate is public, except `run`.
pub use accounts::{NewAccount, create as create_account};
pub use auth::token::{Claims, Keyring, Kind};
pub use cli::{DatabaseOverrides, LogOverrides, Overrides};
pub use config::{LogLevel, Settings, load as load_settings};
pub use http::{AppState, router};
pub use logging::json_subscriber;
pub use store::Store;
pub use store::migrate::Mismatch;
pub use superadmin::reset as reset_superadmin;

use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;

use cli::{Cli, Command};
use setup::SetUp;

/// Runs the subcommand named on the command line and returns its exit status.
/// A failure before the subcommand runs prints its error on stderr and returns
/// failure.
///
/// # Panics
///
/// `run` does not panic.
pub async fn run() -> ExitCode {
    // `run` resolves the settings before any subcommand runs, so a bad
    // configuration fails the same way for every subcommand.
    //
    // 1. We parse the command line. clap handles help, the version and a
    //    usage error itself and exits with status 0 or 2.
    // 2. We resolve the settings. This comes before logging, because the log
    //    level is a setting.
    // 3. We install logging.
    // 4. We warn about each secret flag and log the resolved settings.
    // 5. We dispatch on the subcommand.

    // 1. parse the command line
    let cli = Cli::parse();

    // 2. resolve the settings
    let settings = match config::load(cli.config.as_deref(), &cli.overrides) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    // 3. install logging
    if let Err(error) = logging::init(settings.log.level.0) {
        // The error goes to stderr, because no subscriber exists yet to log
        // it.
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }

    // 4. log the secret flags and the settings
    // A secret on the command line is accepted with a warning, because the
    // process list shows it (ADR-0013).
    for flag in cli.overrides.secrets() {
        tracing::warn!(flag, "a secret setting was given on the command line");
    }
    // One JSON string spares a reader the Debug text. Each value is a string,
    // a table or an absent key, so the serialization cannot fail.
    let json = serde_json::to_string(&settings).expect("the settings serialize");
    tracing::debug!(settings = %json, "settings resolved");

    // 5. dispatch on the subcommand
    match cli.command {
        Command::Serve => serve(&settings).await,
        Command::Migrate => migrate(&settings).await,
        Command::ResetSuperadmin => reset_superadmin_command(&settings).await,
    }
}

/// Serves the API until a signal stops it, after it sets up an empty
/// database. It prints any error on stderr and returns failure.
async fn serve(settings: &Settings) -> ExitCode {
    // Every refusal comes before the listener binds, so a refused server is
    // never reachable.
    //
    // 1. We check that `auth.signing_key` is set.
    // 2. We open the database, setting it up when it is empty.
    // 3. We derive the signing keys, which needs the salt in the database.
    // 4. We serve until a signal.

    // 1. check the signing key
    // The check runs before the database is touched, so a refused start
    // leaves the database as it was.
    let Some(signing) = &settings.auth.signing_key else {
        eprintln!(
            "auth.signing_key is not set; serve needs a passphrase of at least 32 characters, \
             in the configuration file or as TRAPPERKEEPER_AUTH__SIGNING_KEY"
        );
        return ExitCode::FAILURE;
    };

    // 2. open or set up the database
    let store = match setup::open_or_set_up(settings.database.url.expose()).await {
        Ok((store, set_up)) => {
            // Only a database that was empty returns a set-up.
            if let Some(set_up) = &set_up {
                log_set_up(set_up);
            }
            store
        }
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    // 3. derive the keys
    let previous = settings.auth.previous_signing_key.as_ref();
    let keyring = match Keyring::from_store(
        &store,
        signing.expose(),
        previous.map(config::SigningKey::expose),
    )
    .await
    {
        Ok(keyring) => keyring,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    // 4. serve
    let state = AppState {
        store,
        keyring: Arc::new(keyring),
    };
    match http::serve(settings, state).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// Logs what a set-up did: the migration count, and the superadmin's one-time
/// password when the set-up created it, each as one INFO event.
fn log_set_up(set_up: &SetUp) {
    // At info, so an operator sees what the command did.
    tracing::info!(count = set_up.applied, "migrations applied");
    // The password goes out as a field of a log event, so a piped log stream
    // stays JSON lines.
    if let Some(password) = &set_up.password {
        tracing::info!(
            // `$tk` is the name of the system tenant, which the first
            // migration seeds and which holds the superadmin (ADR-0028).
            tenant = "$tk",
            name = superadmin::NAME,
            password,
            "superadmin created"
        );
    }
}

/// Creates the database when it is missing, sets it up, and returns the exit
/// status.
async fn migrate(settings: &Settings) -> ExitCode {
    match setup::create(settings.database.url.expose()).await {
        Ok((_, set_up)) => {
            log_set_up(&set_up);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// Gives the superadmin a new one-time password, creating the superadmin when
/// it is missing, and prints it. It opens the database without creating it.
async fn reset_superadmin_command(settings: &Settings) -> ExitCode {
    let store = match setup::open_checked(settings.database.url.expose()).await {
        Ok(store) => store,
        // The hint stays out of `setup::Error`, because serve shares that
        // text and creates a missing database itself.
        Err(error @ setup::Error::Open(_)) => {
            eprintln!(
                "{error}\nthe database does not open; when it does not exist, \
                 `trapperkeeper serve` or `trapperkeeper migrate` sets it up"
            );
            return ExitCode::FAILURE;
        }
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    match superadmin::reset(&store).await {
        Ok(password) => {
            // The password alone goes to stdout, so a script can read it.
            println!("{password}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
