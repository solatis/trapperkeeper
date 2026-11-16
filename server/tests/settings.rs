//! Tests of the settings loader. One property over generated layers checks
//! the precedence of the listen address, the log level and the database URL,
//! because ADR-0018 prefers a property to a case per layer. Two dedicated
//! tests check that a database URL given as a flag is reported as a secret
//! flag, and that a missing `--config` file is refused.

use std::fmt::Write as _;
use std::path::Path;

use figment::Jail;
use proptest::prelude::*;
use serde_json::Value;
use trapperkeeper_server::{DatabaseOverrides, LogOverrides, Overrides, load_settings};

/// The level names that `LogLevel` parses, in lowercase, as it serializes
/// them.
const LEVELS: [&str; 6] = ["trace", "debug", "info", "warn", "error", "off"];

/// What one settings layer gives for each of the three settings. A `None`
/// field leaves that setting to the layers below.
#[derive(Debug, Clone)]
struct Layer {
    // The port alone, which `address` puts on the loopback host.
    listen: Option<u16>,
    level: Option<&'static str>,
    database: Option<DatabaseUrl>,
}

/// A generated database URL: SQLite, without a password, or Postgres with a
/// password that the resolved settings must mask. The `u8` numbers the
/// database.
#[derive(Debug, Clone)]
enum DatabaseUrl {
    Sqlite(u8),
    Postgres(u8, String),
}

impl DatabaseUrl {
    /// The URL as a layer gives it.
    fn given(&self) -> String {
        // The loader never opens the URL, so nothing dials this host.
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
        // No character of `[a-z0-9]` is reserved in a URL. The url crate drops
        // an empty password, so the length starts at 1, and 12 is a choice.
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
    // The host of the default listen address.
    format!("127.0.0.1:{port}")
}

/// Resolves the three layers through the loader inside a jail and returns the
/// settings as JSON with the flags that carry a secret. Panics when the loader
/// refuses the settings.
fn resolved(file: &Layer, env: &Layer, flags: &Layer) -> (Value, Vec<&'static str>) {
    // The function hands each layer to the loader as the server reads it,
    // inside figment's `Jail`, which locks and restores the environment and
    // the working directory, so parallel tests do not race.
    //
    // 1. We write the file layer as TOML, even when it is empty, so the path
    //    for a named file always runs.
    // 2. We clear the environment, so the developer's shell cannot reach the
    //    loader, and set the environment layer.
    // 3. We build the flags layer as `Overrides`.
    // 4. We load the settings and pass them out as JSON, which masks the
    //    database URL.

    // The jail's closure returns `figment::Result<()>`, so the result leaves
    // through this variable.
    let mut result = None;
    // The jail fixes the closure's error type to figment's, which is over 200
    // bytes, so the lint on large errors is relaxed here.
    #[allow(clippy::result_large_err)]
    Jail::try_with(|jail| {
        // 1. write the file layer
        let mut toml = String::new();
        // `listen` is a top-level key, so it goes before the table headers.
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

        // 2. set the environment layer
        jail.clear_env();
        if let Some(port) = env.listen {
            jail.set_env("TRAPPERKEEPER_LISTEN", address(port));
        }
        if let Some(level) = env.level {
            // A double underscore separates the nested key `log.level`.
            jail.set_env("TRAPPERKEEPER_LOG__LEVEL", level);
        }
        if let Some(url) = &env.database {
            jail.set_env("TRAPPERKEEPER_DATABASE__URL", url.given());
        }

        // 3. build the flags
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

        // 4. load and serialize
        // The loader boxes figment's error, and the jail wants it unboxed.
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
        // Any database URL given as a flag is a secret flag.
        database: Some(DatabaseUrl::Postgres(0, "secret".to_owned())),
        ..silent.clone()
    };
    let (_, secrets) = resolved(&silent, &silent, &flags);
    assert_eq!(secrets, ["--database-url"]);
}

/// A `--config` path that does not exist is refused, and the message names
/// the path. figment reads a missing file as an empty layer, so the refusal
/// comes from the loader's own check, which this test covers.
#[test]
fn a_missing_config_file_is_refused() {
    let path = Path::new("no-such-directory/trapperkeeper.toml");
    // The loader refuses the missing file before it reads the environment,
    // so the test needs no jail.
    let error = load_settings(Some(path), Overrides::default()).err();
    let message = error.expect("a missing file is refused").to_string();
    assert!(
        message.contains("no-such-directory/trapperkeeper.toml"),
        "{message:?}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig {
        // No regression file is left in the tree. The panic message shows the
        // minimal failing input.
        failure_persistence: None,
        // proptest's default number of cases, because a case costs only one
        // file write and one load.
        .. ProptestConfig::default()
    })]

    /// For every setting, the resolved value is the value of the highest
    /// layer that gave one, with the flags above the environment and the
    /// environment above the file, and the default when no layer gave one. A
    /// resolved database URL shows its password as `***`.
    #[test]
    fn the_highest_layer_wins(file in layer(), env in layer(), flags in layer()) {
        let (settings, _) = resolved(&file, &env, &flags);

        // Port 8080 on loopback is the listen address of `Settings::default`.
        let listen = flags.listen.or(env.listen).or(file.listen)
            .map_or_else(|| address(8080), address);
        prop_assert_eq!(settings["listen"].as_str(), Some(listen.as_str()));

        // `info` is the level of `LogSettings::default`.
        let level = flags.level.or(env.level).or(file.level).unwrap_or("info");
        prop_assert_eq!(settings["log"]["level"].as_str(), Some(level));

        // The fallback is the URL of `DatabaseSettings::default`. A given URL
        // is compared masked, as the settings serialize it.
        let database = flags.database.or(env.database).or(file.database)
            .map_or_else(|| "sqlite://trapperkeeper.db".to_owned(), |url| url.masked());
        prop_assert_eq!(settings["database"]["url"].as_str(), Some(database.as_str()));
    }
}
