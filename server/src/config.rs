//! Settings of the server and how they are resolved. Four layers, lowest
//! first: the defaults in `Settings::default`, the TOML file named on the
//! command line, the environment variables prefixed `TRAPPERKEEPER_`, and the
//! command line flags. The last layer merged wins. Where a secret may come
//! from is doc/adr/0013-where-secrets-may-come-from.md, and a secret setting
//! has a type that masks it when serialized or printed.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::str::FromStr;

use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};
use serde::{Deserialize, Serialize, Serializer};
use tracing::metadata::ParseLevelFilterError;
use tracing_subscriber::filter::LevelFilter;
use url::Url;

/// Prefix of every environment variable the server reads, so a generic name
/// such as `LISTEN` on a shared host cannot be mistaken for ours.
pub const ENV_PREFIX: &str = "TRAPPERKEEPER_";

/// Stands for a dot in an environment variable name: `TRAPPERKEEPER_LOG__LEVEL`
/// sets `log.level`. Two underscores, so a single underscore stays usable
/// inside a key name.
pub const ENV_SEPARATOR: &str = "__";

/// Every setting of the server, fully resolved. Each field has a default, so
/// a server with no file, no environment and no flags starts.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Address the server listens on.
    pub listen: SocketAddr,
    /// The log output.
    pub log: LogSettings,
    /// The relational store.
    pub database: DatabaseSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            // Loopback, so a fresh install is not reachable from outside until
            // an operator says so.
            listen: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080),
            log: LogSettings::default(),
            database: DatabaseSettings::default(),
        }
    }
}

/// Settings of the log output.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct LogSettings {
    /// Lowest level written. A `RUST_LOG` directive overrides it.
    pub level: LogLevel,
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            // Info, because the request lines are info and an operator should
            // see them without configuring anything.
            level: LogLevel(LevelFilter::INFO),
        }
    }
}

/// Settings of the relational store.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct DatabaseSettings {
    /// Where the store lives, as a URL sqlx accepts: `sqlite://<path>`,
    /// `postgres://...` or `mariadb://...`.
    pub url: DatabaseUrl,
}

impl Default for DatabaseSettings {
    fn default() -> Self {
        Self {
            // A relative path, so `migrate` and `serve` run from one directory
            // find the same file.
            url: DatabaseUrl("sqlite://trapperkeeper.db".to_owned()),
        }
    }
}

/// A database URL, which may carry a password. It serializes and prints with
/// the password replaced by `***`, and `expose` gives the URL as written
/// (doc/adr/0013).
#[derive(Deserialize, Clone)]
#[serde(transparent)]
pub struct DatabaseUrl(String);

impl DatabaseUrl {
    /// The URL as written, password included, for opening the store.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// The URL with its password replaced by `***`.
    fn masked(&self) -> String {
        match Url::parse(&self.0) {
            Ok(mut url) if url.password().is_some() => {
                // A URL with a password has a host, and only a URL without one
                // refuses a password.
                url.set_password(Some("***"))
                    .expect("a URL with a host takes a password");
                url.to_string()
            }
            // A URL that does not parse is masked whole when it has an `@`,
            // because no one can tell which part is the password. Without an
            // `@` it has no user information, so no password (RFC 3986, 3.2.1).
            Err(_) if self.0.contains('@') => "***".to_owned(),
            Ok(_) | Err(_) => self.0.clone(),
        }
    }
}

impl Serialize for DatabaseUrl {
    // The logged settings are this serialization, so the password never
    // reaches a log line. `load` also serializes the defaults through it, and
    // the default URL holds no password. The flag keeps a plain `String` for
    // the same reason: a masked flag value would replace the real URL in the
    // merge.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.masked())
    }
}

impl fmt::Debug for DatabaseUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.masked())
    }
}

/// A tracing level filter that reads and writes as its name (`info`,
/// `debug`), so the file, the environment and the flag all take the same
/// spelling. It reads any case and writes lowercase, so the logged settings
/// read as a file would be written. A newtype because `LevelFilter` has no
/// serde implementation.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(try_from = "String", into = "String")]
pub struct LogLevel(pub LevelFilter);

impl FromStr for LogLevel {
    type Err = ParseLevelFilterError;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        name.parse().map(Self)
    }
}

impl TryFrom<String> for LogLevel {
    type Error = ParseLevelFilterError;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        name.parse()
    }
}

impl From<LogLevel> for String {
    fn from(level: LogLevel) -> Self {
        level.0.to_string().to_lowercase()
    }
}

/// Why the settings could not be resolved. Boxed because figment's error is
/// over 200 bytes and would otherwise dominate the size of every `Result`
/// that carries it.
pub type Error = Box<figment::Error>;

/// Resolves the settings. `file` is the path given with `--config`, or none;
/// `overrides` is the struct of flags, serialized as the top layer.
///
/// # Errors
///
/// Fails when the file is named but missing, when a layer does not parse, or
/// when a key is unknown or has the wrong type.
pub fn load(file: Option<&Path>, overrides: impl Serialize) -> Result<Settings, Error> {
    // The layers are merged lowest first, and `merge` takes the incoming
    // scalar, so the last layer merged wins: defaults, then file, then
    // environment, then flags. figment's `join` would keep the existing value
    // instead and invert the precedence.
    //
    // 1. Start from the defaults, so every field has a value and a partial
    //    layer above is enough.
    let mut figment = Figment::from(Serialized::defaults(Settings::default()));

    // 2. Add the file only when one was named. A named file that does not
    //    exist is refused here: figment emits an empty dictionary for a
    //    missing file, which would silently run the server on its defaults.
    //    `file_exact` because `file` would also search the parent
    //    directories of a relative path.
    if let Some(path) = file {
        if !path.is_file() {
            return Err(Box::new(figment::Error::from(format!(
                "configuration file {} does not exist",
                path.display()
            ))));
        }
        figment = figment.merge(Toml::file_exact(path));
    }

    // 3. Add the environment, prefixed and split on the separator, so nested
    //    keys are reachable from a flat variable name.
    figment = figment.merge(Env::prefixed(ENV_PREFIX).split(ENV_SEPARATOR));

    // 4. Add the flags last. An absent flag serializes to nothing and leaves
    //    the value below it in place.
    figment = figment.merge(Serialized::defaults(overrides));

    // 5. Extract into `Settings`. `deny_unknown_fields` turns a typo in a key
    //    into an error: the configuration is our own schema and is validated
    //    strictly.
    let settings = figment.extract()?;
    Ok(settings)
}
