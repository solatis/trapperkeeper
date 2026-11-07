//! The server behind the `trapperkeeper` binary. The binary calls `run`, and
//! tests reach the router, the store, the keyring, the superadmin and the
//! creation of users through the items exported here.

mod accounts;
mod auth;
mod cli;
mod config;
mod http;
mod logging;
mod names;
mod scope;
mod sessions;
mod store;
mod superadmin;
mod tenants;
mod users;

// The integration tests build a migrated store with its superadmin, derive a
// keyring from it, serve the router on a listener they bound, create users
// of any role, and mint tokens of their own. Nothing else in the crate is
// part of its interface.
pub use accounts::{NewAccount, create as create_account};
pub use auth::token::{Claims, Keyring, Kind};
pub use http::{AppState, router};
pub use store::migrate::apply as apply_migrations;
pub use store::{Store, create as create_store};
pub use superadmin::reset as reset_superadmin;

use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;

use cli::{Cli, Command};
use config::Settings;
use store::migrate::Mismatch;

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
    // process list shows it (doc/adr/0013).
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
    //    no default, because a default holds no secret (doc/adr/0013).
    let Some(signing) = &settings.auth.signing_key else {
        eprintln!(
            "auth.signing_key is not set; serve needs a passphrase of at least 32 characters, \
             in the configuration file or as TRAPPERKEEPER_AUTH__SIGNING_KEY"
        );
        return ExitCode::FAILURE;
    };

    // 2.
    let Some(store) = open_for_serve(settings).await else {
        return ExitCode::FAILURE;
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

/// Opens the database for serve and checks it against the binary's
/// migrations. Sets up an empty database first. Returns `None` after printing
/// the refusal.
async fn open_for_serve(settings: &Settings) -> Option<Store> {
    // 1. open the database, and create it only when it does not exist
    // 2. check it against the migrations
    // 3. set up an empty database

    // 1. Open is tried first, so a serve whose user may not create databases
    //    still opens an existing one.
    let url = settings.database.url.expose();
    let store = match store::open(url).await {
        Ok(store) => store,
        // A database that exists, or whose existence cannot be asked, is
        // refused with the open's error.
        Err(error) if !matches!(store::exists(url).await, Ok(false)) => {
            eprintln!("{error}");
            return None;
        }
        Err(_) => match store::create(url).await {
            Ok(store) => store,
            Err(error) => {
                eprintln!("{error}");
                return None;
            }
        },
    };

    // 2. and 3. An empty database is migrated and given its superadmin, and
    //    every other mismatch refuses.
    match store::migrate::check(&store).await {
        Ok(None) => Some(store),
        Ok(Some(Mismatch::Empty)) => set_up(&store).await.then_some(store),
        Ok(Some(mismatch)) => {
            eprintln!("{mismatch}");
            None
        }
        Err(error) => {
            eprintln!("{error}");
            None
        }
    }
}

/// Opens the database without creating it and checks it against the binary's
/// migrations. Returns `None` after printing the refusal.
async fn open_checked(settings: &Settings) -> Option<Store> {
    let store = match store::open(settings.database.url.expose()).await {
        Ok(store) => store,
        Err(error) => {
            eprintln!(
                "{error}\nthe database does not open; when it does not exist, \
                 `trapperkeeper serve` or `trapperkeeper migrate` sets it up"
            );
            return None;
        }
    };
    match store::migrate::check(&store).await {
        Ok(None) => Some(store),
        Ok(Some(mismatch)) => {
            eprintln!("{mismatch}");
            None
        }
        Err(error) => {
            eprintln!("{error}");
            None
        }
    }
}

/// Applies the migrations, creates the superadmin when the database was
/// empty, and logs the superadmin's one-time password. Returns whether both
/// succeeded, after printing any error.
async fn set_up(store: &Store) -> bool {
    match store::migrate::apply(store).await {
        // At info, so an operator sees what the command did.
        Ok(count) => tracing::info!(count, "migrations applied"),
        Err(error) => {
            eprintln!("{error}");
            return false;
        }
    }
    // After the migrations, because they seed `$tk` and its role.
    match superadmin::ensure(store).await {
        // One INFO event with the password as a field, so a piped log stream
        // stays JSON lines.
        Ok(Some(password)) => {
            tracing::info!(
                tenant = "$tk",
                name = superadmin::NAME,
                password,
                "superadmin created"
            );
            true
        }
        Ok(None) => true,
        Err(error) => {
            eprintln!("{error}");
            false
        }
    }
}

/// Creates the database when it is missing, sets it up, and returns the exit
/// status.
async fn migrate(settings: &Settings) -> ExitCode {
    let store = match store::create(settings.database.url.expose()).await {
        Ok(store) => store,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    if set_up(&store).await {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Gives the superadmin a new one-time password, prints it, and returns the
/// exit status.
async fn reset_superadmin_command(settings: &Settings) -> ExitCode {
    let Some(store) = open_checked(settings).await else {
        return ExitCode::FAILURE;
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
