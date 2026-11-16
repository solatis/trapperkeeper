//! The server's settings and their resolution by `load`. ADR-0013 decides
//! where a secret setting may come from. The database URL and the signing
//! keys are the secret settings, and each has a type that masks it when
//! serialized or printed.

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

/// Prefix of every environment variable that sets a setting, so a generic
/// name such as `LISTEN` on a shared host cannot be mistaken for ours. The
/// server also reads `RUST_LOG`, which has no prefix.
pub const ENV_PREFIX: &str = "TRAPPERKEEPER_";

/// Stands for a dot in an environment variable name: `TRAPPERKEEPER_LOG__LEVEL`
/// sets `log.level`. Two underscores, so a single underscore stays usable
/// inside a key name.
pub const ENV_SEPARATOR: &str = "__";

/// Every setting of the server, fully resolved. Each field has a default, so
/// the settings resolve with no file, no environment and no flags. `serve`
/// still refuses to start without `auth.signing_key`.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Address the server listens on.
    pub listen: SocketAddr,
    /// The log output.
    pub log: LogSettings,
    /// The relational store.
    pub database: DatabaseSettings,
    /// Authentication of people.
    pub auth: AuthSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            // Loopback, so a fresh install is not reachable from outside until
            // an operator says so. The port 8080 is a choice that no
            // measurement backs.
            listen: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080),
            log: LogSettings::default(),
            database: DatabaseSettings::default(),
            auth: AuthSettings::default(),
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
            // The default is the SQLite file `trapperkeeper.db` in the working
            // directory, so a first run needs no configuration (ADR-0019).
            url: DatabaseUrl("sqlite://trapperkeeper.db".to_owned()),
        }
    }
}

/// A database URL, which may carry a password. It serializes and prints with
/// the password replaced by `***`, and `expose` gives the URL as written
/// (ADR-0013). A URL that does not parse and contains an `@` serializes and
/// prints as `***` whole. `run` logs the resolved settings as their JSON
/// serialization, so the password never reaches a log line.
#[derive(Deserialize, Clone)]
#[serde(transparent)]
pub struct DatabaseUrl(String);

impl DatabaseUrl {
    /// The URL as written, password included, for opening the store.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// The URL with its password replaced by `***`. A URL that does not parse
    /// and contains an `@` comes back as `***` whole, and every other URL
    /// without a password comes back unchanged.
    fn masked(&self) -> String {
        match Url::parse(&self.0) {
            Ok(mut url) if url.password().is_some() => {
                // A URL with a password has a host, and only a URL without one
                // refuses a password.
                url.set_password(Some("***"))
                    .expect("a URL with a host takes a password");
                url.to_string()
            }
            // An unparsed URL with an `@` is masked whole, because no one can
            // tell which part is the password.
            Err(_) if self.0.contains('@') => "***".to_owned(),
            // This arm takes a parsed URL without a password, such as
            // `sqlite://trapperkeeper.db`, and an unparsed URL without an `@`.
            // The second has no user information (RFC 3986, 3.2.1).
            Ok(_) | Err(_) => self.0.clone(),
        }
    }
}

impl Serialize for DatabaseUrl {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.masked())
    }
}

impl fmt::Debug for DatabaseUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.masked())
    }
}

/// Settings of authentication. The default holds neither key. `serve` refuses
/// to start without `signing_key`, and migrate and reset-superadmin need no
/// key.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct AuthSettings {
    /// The passphrase that signs new tokens and checks them.
    pub signing_key: Option<SigningKey>,
    /// The passphrase that signed before the last rotation. It only checks
    /// tokens, and it checks them for as long as it is set. The operator
    /// removes it once the tokens it signed have expired, 7 days after the
    /// rotation, which is the lifetime of a refresh token. The server does not
    /// drop it by itself.
    pub previous_signing_key: Option<SigningKey>,
}

/// A signing passphrase as typed in the configuration: any text of at least
/// 32 characters. It serializes and prints as `***` (ADR-0013). `run` logs the
/// resolved settings as their JSON serialization, so no signing key reaches a
/// log line.
#[derive(Deserialize, Clone)]
#[serde(try_from = "String")]
pub struct SigningKey(String);

impl SigningKey {
    /// The passphrase as typed, for deriving the key.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SigningKey {
    type Error = String;

    fn try_from(passphrase: String) -> Result<Self, Self::Error> {
        // 32 characters is about six words, so a passphrase typed by hand
        // still resists offline guessing once stretched.
        if passphrase.chars().count() < 32 {
            return Err("a signing key needs at least 32 characters".to_owned());
        }
        Ok(Self(passphrase))
    }
}

impl Serialize for SigningKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("***")
    }
}

impl fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
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
        level.0.to_string()
    }
}

/// Why the settings could not be resolved. Boxed because figment's error is
/// over 200 bytes and would otherwise dominate the size of every `Result`
/// that carries it.
pub type Error = Box<figment::Error>;

/// Resolves the settings from four sources: the defaults, the TOML file at
/// `file` when one is given, the environment variables prefixed
/// `TRAPPERKEEPER_`, and `overrides`. A later source wins over an earlier one.
/// `overrides` must serialize to the tree of `Settings`, with each absent value
/// omitted.
///
/// # Errors
///
/// Fails when `file` names no file, which includes a missing path and a
/// directory, when a layer does not parse, when a key is unknown, when a value
/// has the wrong type, or when its type refuses it, such as a signing key
/// shorter than 32 characters or an unknown level name.
pub fn load(file: Option<&Path>, overrides: impl Serialize) -> Result<Settings, Error> {
    // `load` merges the layers with figment from the lowest up, and then
    // extracts the result into `Settings`. figment's `merge` takes the
    // incoming value, so the layer merged last wins. `join` would keep the
    // existing value instead and invert the precedence.
    //
    // 1. We start from the serialized defaults of `Settings`. Every field then
    //    has a value, so a layer above may set only some keys.
    // 2. We merge the TOML file when a path was given, and refuse a path that
    //    names no file.
    // 3. We merge the environment variables that carry the prefix, split on
    //    the separator, so a flat name such as `TRAPPERKEEPER_LOG__LEVEL`
    //    reaches the nested key `log.level`.
    // 4. We merge the serialized flags last, so a given flag wins over every
    //    other layer. An absent flag serializes to nothing and leaves the
    //    value below it in place.
    // 5. We extract the merged tree into `Settings`, where an unknown key or a
    //    value that its type refuses fails. `deny_unknown_fields` turns a typo
    //    in a key into an error.

    // 1. start from the defaults
    // The defaults pass through the masking serializers of `DatabaseUrl` and
    // `SigningKey`. No default holds a secret, so the masks change nothing.
    let mut figment = Figment::from(Serialized::defaults(Settings::default()));

    // 2. merge the file
    if let Some(path) = file {
        // The guard refuses a path that names no file before figment reads
        // it, with a message that names the path.
        if !path.is_file() {
            return Err(Box::new(figment::Error::from(format!(
                "configuration file {} does not exist",
                path.display()
            ))));
        }
        // The call uses `file_exact`, because `file` would also search the
        // parent directories of a relative path.
        figment = figment.merge(Toml::file_exact(path));
    }

    // 3. merge the environment
    figment = figment.merge(Env::prefixed(ENV_PREFIX).split(ENV_SEPARATOR));

    // 4. merge the flags
    // `defaults` names figment's default profile, the only profile this
    // server uses.
    figment = figment.merge(Serialized::defaults(overrides));

    // 5. extract the settings
    let settings = figment.extract()?;
    Ok(settings)
}
