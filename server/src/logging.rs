//! Log output of the binary: which lines it writes and in which format.
//! ADR-0017 sets these rules, and this module carries them out.

use std::env;
use std::fmt;
use std::io::{self, IsTerminal};

use tracing::Subscriber;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::{FromEnvError, LevelFilter};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::util::{SubscriberInitExt, TryInitError};

/// Name of the environment variable whose filter directive, when set,
/// replaces the log level setting.
const DIRECTIVE_VAR: &str = "RUST_LOG";

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

/// The JSON subscriber: the events `filter` passes, each written as one JSON
/// line to `writer`.
pub fn json_subscriber<W>(filter: EnvFilter, writer: W) -> impl Subscriber + Send + Sync + 'static
where
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .with_writer(writer)
        .finish()
}

/// Installs the global tracing subscriber on stdout at `level`, the resolved
/// `log.level` setting. A set `RUST_LOG` directive replaces `level`.
///
/// # Errors
///
/// Fails when `RUST_LOG` is set to an invalid directive, or when a subscriber
/// is already installed.
pub fn init(level: LevelFilter) -> Result<(), Error> {
    // The directive wins over the level, because its syntax can raise the
    // level of one crate for a debugging run.
    let filter = if env::var_os(DIRECTIVE_VAR).is_some() {
        // An invalid directive fails, because a fallback to a default filter
        // would let a typo log silently at the setting's level.
        EnvFilter::try_from_env(DIRECTIVE_VAR).map_err(Error::InvalidDirective)?
    } else {
        EnvFilter::default().add_directive(level.into())
    };

    // A terminal gets the compact colored format and a pipe gets JSON lines.
    // Detection serves both a person and a collector, so the format has no
    // setting (ADR-0017).
    if io::stdout().is_terminal() {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .compact()
            .finish()
            .try_init()
    } else {
        json_subscriber(filter, io::stdout).try_init()
    }
    .map_err(Error::AlreadyInstalled)
}
