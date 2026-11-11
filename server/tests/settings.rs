//! The precedence of the settings layers through the loader. Three settings
//! are covered: the listen address, the log level and the database URL. For
//! each, the file, the environment and the flags each give a random value or
//! stay silent. A database URL is a SQLite URL or a Postgres URL with a
//! generated password. Each resolved setting must be the value of the highest
//! layer that gave one, or the default when none did, and a resolved URL
//! serializes with its password replaced by `***`. One property over
//! generated layers instead of a case per layer, as ADR-0018 asks, each
//! case inside figment's jail.

use std::fmt::Write as _;
use std::path::Path;

use figment::Jail;
use proptest::prelude::*;
use serde_json::Value;
use trapperkeeper_server::{DatabaseOverrides, LogOverrides, Overrides, load_settings};

/// The level names the settings accept, as a file would spell them.
const LEVELS: [&str; 6] = ["trace", "debug", "info", "warn", "error", "off"];

/// What one layer says about each setting; `None` leaves the setting to the
/// layers below.
#[derive(Debug, Clone)]
struct Layer {
    listen: Option<u16>,
    level: Option<&'static str>,
    database: Option<DatabaseUrl>,
}

/// A generated database URL.
#[derive(Debug, Clone)]
enum DatabaseUrl {
    Sqlite(u8),
    Postgres(u8, String),
}

impl DatabaseUrl {
    /// The URL as a layer gives it.
    fn given(&self) -> String {
        match self {
            Self::Sqlite(n) => format!("sqlite://db-{n}.db"),
            Self::Postgres(n, password) => format!("postgres://tk:{password}@127.0.0.1:1/db-{n}"),
        }
    }

    /// The URL as the resolved settings must show it.
    fn masked(&self) -> String {
        match self {
            Self::Sqlite(_) => self.given(),
            Self::Postgres(n, _) => format!("postgres://tk:***@127.0.0.1:1/db-{n}"),
        }
    }
}

fn database_url() -> impl Strategy<Value = DatabaseUrl> {
    prop_oneof![
        any::<u8>().prop_map(DatabaseUrl::Sqlite),
        (any::<u8>(), "[a-z0-9]{1,12}").prop_map(|(n, p)| DatabaseUrl::Postgres(n, p)),
    ]
}

fn layer() -> impl Strategy<Value = Layer> {
    (
        proptest::option::of(any::<u16>()),
        proptest::option::of(proptest::sample::select(&LEVELS[..])),
        proptest::option::of(database_url()),
    )
        .prop_map(|(listen, level, database)| Layer {
            listen,
            level,
            database,
        })
}

fn address(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

/// Resolves the three layers through the loader inside a jail and returns the
/// settings as JSON with the flags that carry a secret.
fn resolved(file: &Layer, env: &Layer, flags: &Layer) -> (Value, Vec<&'static str>) {
    // 1. write the file layer as TOML into the jail's directory
    // 2. clear the environment and set the environment layer
    // 3. build the overrides from the flags layer
    // 4. load, and serialize the settings to JSON

    let mut result = None;
    // The jail fixes the closure's error type to figment's, which is over 200
    // bytes, so the lint on large errors is relaxed here.
    #[allow(clippy::result_large_err)]
    Jail::try_with(|jail| {
        // 1. An empty file still exists, so the file path is always exercised.
        let mut toml = String::new();
        if let Some(port) = file.listen {
            writeln!(toml, "listen = \"{}\"", address(port)).expect("a string grows");
        }
        if let Some(level) = file.level {
            writeln!(toml, "[log]\nlevel = \"{level}\"").expect("a string grows");
        }
        if let Some(url) = &file.database {
            writeln!(toml, "[database]\nurl = \"{}\"", url.given()).expect("a string grows");
        }
        jail.create_file("trapperkeeper.toml", &toml)?;
        let path = jail.directory().join("trapperkeeper.toml");

        // 2. The jail locks the environment and the working directory while
        //    the case runs and restores both after, so parallel tests do not
        //    race. The inherited environment is cleared first, so a
        //    developer's shell cannot leak into the property.
        jail.clear_env();
        if let Some(port) = env.listen {
            jail.set_env("TRAPPERKEEPER_LISTEN", address(port));
        }
        if let Some(level) = env.level {
            jail.set_env("TRAPPERKEEPER_LOG__LEVEL", level);
        }
        if let Some(url) = &env.database {
            jail.set_env("TRAPPERKEEPER_DATABASE__URL", url.given());
        }

        // 3.
        let overrides = Overrides {
            listen: flags
                .listen
                .map(|port| address(port).parse().expect("a socket address")),
            log: LogOverrides {
                level: flags.level.map(|level| level.parse().expect("a level")),
            },
            database: DatabaseOverrides {
                url: flags.database.as_ref().map(DatabaseUrl::given),
            },
        };

        // 4.
        let settings = load_settings(Some(&path), &overrides).map_err(|error| *error)?;
        let json = serde_json::to_value(&settings).expect("the settings serialize");
        result = Some((json, overrides.secrets()));
        Ok(())
    })
    .expect("the settings resolve");
    result.expect("the jail ran the case")
}

/// A database URL given as a flag is reported as a secret flag.
#[test]
fn a_secret_flag_is_reported() {
    let silent = Layer {
        listen: None,
        level: None,
        database: None,
    };
    let flags = Layer {
        database: Some(DatabaseUrl::Postgres(0, "secret".to_owned())),
        ..silent.clone()
    };
    let (_, secrets) = resolved(&silent, &silent, &flags);
    assert_eq!(secrets, ["--database-url"]);
}

/// A `--config` path that does not exist is refused, and the message names
/// the path.
#[test]
fn a_missing_config_file_is_refused() {
    let path = Path::new("no-such-directory/trapperkeeper.toml");
    let error = load_settings(Some(path), Overrides::default()).err();
    // A dedicated case, because figment reads a missing file as empty, and
    // the refusal is this repository's guard.
    let message = error.expect("a missing file is refused").to_string();
    assert!(
        message.contains("no-such-directory/trapperkeeper.toml"),
        "{message:?}"
    );
}

proptest! {
    // proptest's default case count, because a case costs one file write and
    // one load. No regression file, so a failure leaves nothing untracked in
    // the tree; the failing seed is in the output.
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// For every setting, the resolved value is the highest layer's, flags
    /// over environment over file, and the default when every layer is silent.
    #[test]
    fn the_highest_layer_wins(file in layer(), env in layer(), flags in layer()) {
        let (settings, _) = resolved(&file, &env, &flags);

        let listen = flags.listen.or(env.listen).or(file.listen)
            .map_or_else(|| address(8080), address);
        prop_assert_eq!(settings["listen"].as_str(), Some(listen.as_str()));

        let level = flags.level.or(env.level).or(file.level).unwrap_or("info");
        prop_assert_eq!(settings["log"]["level"].as_str(), Some(level));

        let database = flags.database.or(env.database).or(file.database)
            .map_or_else(|| "sqlite://trapperkeeper.db".to_owned(), |url| url.masked());
        prop_assert_eq!(settings["database"]["url"].as_str(), Some(database.as_str()));
    }
}
