//! Log output of the binary: which lines are written and in which format. The
//! rules are doc/adr/0017-tracing-for-structured-logs.md.

use std::env;
use std::fmt;
use std::io::{self, IsTerminal};

use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::{FromEnvError, LevelFilter};
use tracing_subscriber::util::{SubscriberInitExt, TryInitError};

/// Name of the environment variable whose filter directive, when set,
/// replaces the log level setting.
pub const DIRECTIVE_VAR: &str = "RUST_LOG";

/// Why the subscriber could not be installed.
#[derive(Debug)]
pub enum Error {
    /// `RUST_LOG` is set and does not parse as a filter directive.
    InvalidDirective(FromEnvError),
    /// A subscriber is already installed; a programming error.
    AlreadyInstalled(TryInitError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDirective(error) => write!(
                f,
                "{DIRECTIVE_VAR} is not a valid filter directive: {error}"
            ),
            Self::AlreadyInstalled(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for Error {}

/// Installs the global tracing subscriber, writing to stdout. `level` is the
/// resolved `log.level` setting.
///
/// # Errors
///
/// Fails when `RUST_LOG` is set to an invalid directive, or when a subscriber
/// is already installed.
pub fn init(level: LevelFilter) -> Result<(), Error> {
    // Two decisions, each with a rejected alternative:
    //
    // 1. which events pass the filter;
    // 2. in which format they are written.

    // 1. A RUST_LOG directive, when the variable is present, replaces the
    //    setting, because its syntax can raise one crate's level for
    //    debugging. It is parsed strictly: the usual idiom of falling back to
    //    a default filter when it does not parse would let a typo silently log
    //    at the setting's level. Otherwise the filter is the level alone.
    let filter = if env::var_os(DIRECTIVE_VAR).is_some() {
        EnvFilter::try_from_default_env().map_err(Error::InvalidDirective)?
    } else {
        EnvFilter::default().add_directive(level.into())
    };

    // 2. JSON when stdout is not a terminal, the compact format with colors
    //    when it is. `IsTerminal` on stdout decides and there is no setting: a
    //    collector reads a pipe, a person reads a terminal, and detection
    //    covers both without a switch.
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    if io::stdout().is_terminal() {
        builder.compact().finish().try_init()
    } else {
        builder.json().finish().try_init()
    }
    .map_err(Error::AlreadyInstalled)
}
