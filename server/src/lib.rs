//! The server behind the `trapperkeeper` binary. The binary calls `run`, and
//! the tests reach the set-up of a database, the settings, the router, the
//! store, the keyring, the superadmin, the JSON subscriber and the creation
//! of users through the items exported here.

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

// The integration tests set up a database the way the commands do, resolve
// settings the way `run` does, install the JSON subscriber on a buffer,
// derive a keyring from a store, serve the router on a listener they bound,
// create users of any role, and mint tokens of their own. Nothing else in the
// crate is part of its interface.
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
/// Everything below is async wherever an async form exists.
///
/// # Panics
///
/// Never in practice: the settings are plain structs and strings, which always
/// serialize to JSON.
pub async fn run() -> ExitCode {
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

    // 3. Install logging at the resolved level. Its error goes to stderr,
    //    because no subscriber exists yet to log it.
    if let Err(error) = logging::init(settings.log.level.0) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    // A secret on the command line is accepted with a warning, because the
    // process list shows it (ADR-0013).
    for flag in cli.overrides.secrets() {
        tracing::warn!(flag, "a secret setting was given on the command line");
    }
    // One JSON string, so a support case and the settings test read what the
    // server resolved without parsing Debug text. Secrets serialize masked.
    let json = serde_json::to_string(&settings).expect("the settings serialize");
    tracing::debug!(settings = %json, "settings resolved");

    // 4. Dispatch on the subcommand.
    match cli.command {
        Command::Serve => serve(&settings).await,
        Command::Migrate => migrate(&settings).await,
        Command::ResetSuperadmin => reset_superadmin_command(&settings).await,
    }
}

/// Checks the signing key, opens the database or sets it up when it is
/// empty, derives the signing keys, serves until a signal, and returns the
/// exit status.
async fn serve(settings: &Settings) -> ExitCode {
    // 1. check that the signing key is set
    // 2. open the database, setting it up when it is empty
    // 3. derive the keys from the passphrases and the database's salt
    // 4. serve with the store and the keys
    //
    // Every refusal comes before the listener binds, so a refused server is
    // never reachable.

    // 1. Checked before the database is touched, so a refused first start
    //    does not create a superadmin whose password nobody sees. The key has
    //    no default, because a default holds no secret (ADR-0013).
    let Some(signing) = &settings.auth.signing_key else {
        eprintln!(
            "auth.signing_key is not set; serve needs a passphrase of at least 32 characters, \
             in the configuration file or as TRAPPERKEEPER_AUTH__SIGNING_KEY"
        );
        return ExitCode::FAILURE;
    };

    // 2.
    let store = match setup::open_or_set_up(settings.database.url.expose()).await {
        Ok((store, set_up)) => {
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

    // 3.
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

    // 4.
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
    // One INFO event with the password as a field, so a piped log stream
    // stays JSON lines.
    if let Some(password) = &set_up.password {
        tracing::info!(
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

/// Gives the superadmin a new one-time password, prints it, and returns the
/// exit status.
async fn reset_superadmin_command(settings: &Settings) -> ExitCode {
    let store = match setup::open_checked(settings.database.url.expose()).await {
        Ok(store) => store,
        // The hint is this command's and not the error's, because
        // reset-superadmin is the command that never creates a database.
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
            // println and not tracing: the command's output is the password
            // alone, so a script can read it, and it is never a log event.
            println!("{password}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
