//! Log output of the binary: which lines are written and in which format. The
//! rules are doc/adr/0017-tracing-for-structured-logs.md.

use std::io::{self, IsTerminal};

use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::util::{SubscriberInitExt, TryInitError};

/// Installs the global tracing subscriber, writing to stdout at `level` and
/// above.
///
/// # Errors
///
/// Fails when a subscriber is already installed.
pub fn init(level: LevelFilter) -> Result<(), TryInitError> {
    // Two decisions, each with a rejected alternative:
    //
    // 1. which events pass the filter;
    // 2. in which format they are written.

    // 1. The filter is the level alone.
    let filter = EnvFilter::default().add_directive(level.into());

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
}
