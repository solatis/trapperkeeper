//! Settings of the server and how they are resolved. Two layers today, lowest
//! first: the defaults in `Settings::default`, then the TOML file named on the
//! command line. The environment and the command line flags are layered on
//! top by the next commit.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;

use figment::Figment;
use figment::providers::{Format, Serialized, Toml};
use serde::{Deserialize, Serialize};

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

/// Resolves the settings. `file` is the path given with `--config`, or none.
///
/// # Errors
///
/// Fails when the file is named but missing, when a layer does not parse, or
/// when a key is unknown or has the wrong type.
pub fn load(file: Option<&Path>) -> Result<Settings, Error> {
    // The layers are merged lowest first, and `merge` takes the incoming
    // scalar, so the last layer merged wins.
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

    // 3. Extract into `Settings`. `deny_unknown_fields` turns a typo in a key
    //    into an error: the configuration is our own schema and is validated
    //    strictly.
    let settings = figment.extract()?;
    Ok(settings)
}
