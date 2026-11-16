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
    // `run` resolves everything a subcommand needs and then dispatches to it.
    // It resolves the settings before it installs logging and before any
    // subcommand runs, so a bad configuration fails the same way for every
    // subcommand.
    //
    // 1. We parse the command line with clap. clap prints help, the version
    //    or a usage error itself and exits the process, with status 0 for
    //    help and the version and status 2 for a usage error, so no parse
    //    error reaches `run`.
    // 2. We resolve the settings from the file that `--config` names, the
    //    environment and the flags, and on an error we print it and return
    //    failure. This comes before logging, because the log level is one of
    //    the settings.
    // 3. We install logging at the resolved level, and on an error we print
    //    it and return failure.
    // 4. We log a warning for each secret given as a flag, and we log the
    //    resolved settings at debug. Both are log events, so they come after
    //    step 3.
    // 5. We dispatch on the subcommand and return the exit status that it
    //    returns.

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
    // The settings go out as one JSON string, so a reader of the log sees
    // what the server resolved without parsing Debug text. Each value is a
    // string, a table or an absent key, so the serialization cannot fail.
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
/// database, and returns success. It prints the error on stderr and returns
/// failure when the signing key is not set, when the database is refused or
/// cannot be set up, when the keys cannot be derived, or when installing the
/// signal handlers, binding or serving fails.
async fn serve(settings: &Settings) -> ExitCode {
    // `serve` refuses to start at every check before `http::serve` binds the
    // listener, so a refused server is never reachable.
    //
    // 1. We check that `auth.signing_key` is set, and refuse to start with a
    //    message on stderr when it is not.
    // 2. We open the database, set it up when it is empty and log what the
    //    set-up did, or print the error and fail.
    // 3. We derive the signing keys from the current and the previous
    //    passphrase and the salt that the database holds. This comes after
    //    step 2, because the salt lives in the store.
    // 4. We build the state of the router from the store and the keys, and
    //    serve until a signal, returning success or printing the error.

    // 1. check the signing key
    // The check runs before the database is touched, so a refused start
    // leaves the database as it was, and the first start that succeeds is the
    // one that sets it up and logs the one-time password.
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
            // `open_or_set_up` returns a set-up only when it found the
            // database empty and set it up. A database that already matches
            // the migrations returns none.
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
/// it is missing, prints the password, and returns the exit status. It opens
/// the database without creating it, so it fails when the database does not
/// open or does not match the migrations.
async fn reset_superadmin_command(settings: &Settings) -> ExitCode {
    let store = match setup::open_checked(settings.database.url.expose()).await {
        Ok(store) => store,
        // The command adds the hint here and not in the `Display` of
        // `setup::Error`, because serve shares that text and creates a missing
        // database itself. Only this command opens without creating.
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
            // The command prints the password alone on stdout instead of
            // logging it, so a script can read the output.
            println!("{password}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
