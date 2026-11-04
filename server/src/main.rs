//! Entry point of the `trapperkeeper` binary. The tokio runtime owns the
//! process, and the library does the rest.

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    trapperkeeper_server::run().await
}
