//! Settings of the server and how they are resolved. Four layers, lowest
//! first: the defaults in `Settings::default`, the TOML file named on the
//! command line, the environment variables prefixed `TRAPPERKEEPER_`, and the
//! command line flags. The last layer merged wins.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;

use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};
use serde::{Deserialize, Serialize};

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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            // Loopback, so a fresh install is not reachable from outside until
            // an operator says so.
            listen: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080),
        }
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
    // environment, then flags.
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

#[cfg(test)]
// `Jail::expect_with` fixes the closure's error type to figment's unboxed
// error, so the large-error lint cannot be satisfied here.
#[expect(
    clippy::result_large_err,
    reason = "the closure type is figment::Jail's, not ours"
)]
mod tests {
    use std::net::SocketAddr;

    use figment::Jail;

    use super::load;
    use crate::cli::Overrides;

    // Every test runs inside a `Jail`: it gives the test its own directory and
    // environment, undone when the jail drops, and serializes the jails, which
    // matters because cargo runs tests on parallel threads and the environment
    // and the working directory are process-wide. Setting a variable directly
    // is `unsafe` in edition 2024 for the same reason.

    const FILE: &str = "trapperkeeper.toml";

    fn addr(port: u16) -> SocketAddr {
        format!("127.0.0.1:{port}")
            .parse()
            .expect("a socket address")
    }

    fn overrides(listen: Option<u16>) -> Overrides {
        Overrides {
            listen: listen.map(addr),
        }
    }

    #[test]
    fn defaults_alone() {
        Jail::expect_with(|_| {
            let settings = load(None, Overrides::default()).expect("the defaults load");
            assert_eq!(settings.listen, addr(8080));
            Ok(())
        });
    }

    #[test]
    fn file_sets_listen() {
        Jail::expect_with(|jail| {
            jail.create_file(FILE, "listen = \"127.0.0.1:1\"")?;
            let settings = load(Some(&jail.directory().join(FILE)), Overrides::default())
                .expect("the file loads");
            assert_eq!(settings.listen, addr(1));
            Ok(())
        });
    }

    #[test]
    fn env_beats_file() {
        Jail::expect_with(|jail| {
            jail.create_file(FILE, "listen = \"127.0.0.1:1\"")?;
            jail.set_env("TRAPPERKEEPER_LISTEN", "127.0.0.1:2");
            let settings = load(Some(&jail.directory().join(FILE)), Overrides::default())
                .expect("the layers load");
            assert_eq!(settings.listen, addr(2));
            Ok(())
        });
    }

    #[test]
    fn flag_beats_env() {
        Jail::expect_with(|jail| {
            jail.create_file(FILE, "listen = \"127.0.0.1:1\"")?;
            jail.set_env("TRAPPERKEEPER_LISTEN", "127.0.0.1:2");
            let settings = load(Some(&jail.directory().join(FILE)), overrides(Some(3)))
                .expect("the layers load");
            assert_eq!(settings.listen, addr(3));
            Ok(())
        });
    }

    #[test]
    fn absent_flag_keeps_env() {
        Jail::expect_with(|jail| {
            jail.set_env("TRAPPERKEEPER_LISTEN", "127.0.0.1:2");
            let settings = load(None, overrides(None)).expect("the layers load");
            assert_eq!(settings.listen, addr(2));
            Ok(())
        });
    }

    #[test]
    fn missing_file_fails() {
        Jail::expect_with(|jail| {
            let path = jail.directory().join("absent.toml");
            let error = load(Some(&path), Overrides::default()).expect_err("a missing file fails");
            assert!(error.to_string().contains("absent.toml"));
            Ok(())
        });
    }

    #[test]
    fn unknown_key_fails() {
        Jail::expect_with(|jail| {
            jail.create_file(FILE, "listne = \"127.0.0.1:1\"")?;
            load(Some(&jail.directory().join(FILE)), Overrides::default())
                .expect_err("an unknown key fails");
            Ok(())
        });
    }

    #[test]
    fn bad_address_fails() {
        Jail::expect_with(|jail| {
            jail.set_env("TRAPPERKEEPER_LISTEN", "nope");
            load(None, Overrides::default()).expect_err("a bad address fails");
            Ok(())
        });
    }
}
