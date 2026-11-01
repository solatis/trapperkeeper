//! Settings of the server and how they are resolved. Today the only layer is
//! the defaults in `Settings::default`; the configuration file, the
//! environment and the command line flags are layered on top by the next
//! commits.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use figment::Figment;
use figment::providers::Serialized;
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

/// Resolves the settings.
///
/// # Errors
///
/// Fails when a layer does not parse, or when a key is unknown or has the
/// wrong type.
pub fn load() -> Result<Settings, Error> {
    // The layers are merged lowest first, and `merge` takes the incoming
    // scalar, so the last layer merged wins.
    //
    // 1. Start from the defaults, so every field has a value and a partial
    //    layer above is enough.
    let settings = Figment::from(Serialized::defaults(Settings::default()))
        // 2. Extract into `Settings`. `deny_unknown_fields` turns a typo in a
        //    key into an error: the configuration is our own schema and is
        //    validated strictly.
        .extract()?;
    Ok(settings)
}
