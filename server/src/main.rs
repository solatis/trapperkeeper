//! Entry point of the `trapperkeeper` binary. `main` starts the tokio runtime
//! and returns the exit status of `trapperkeeper_server::run`. The work lives
//! in the library, where the integration tests reach it.

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    trapperkeeper_server::run().await
}
