//! The HTTP server: the router and how it is served and stopped. The routes
//! arrive with the next commits.

use std::io;

use axum::Router;
use tokio::net::TcpListener;

use crate::config::Settings;

/// Builds the router with every route. Empty until the next commit.
fn router() -> Router {
    Router::new()
}

/// Serves the router on the configured address until a shutdown signal, then
/// finishes the in-flight requests and returns.
///
/// # Errors
///
/// An I/O error binding or serving is returned, not printed.
pub async fn serve(settings: &Settings) -> io::Result<()> {
    // Bind, then serve until told to stop: a sequence with its own failure at
    // each step.
    //
    // 1. Bind the listen address. Failure here is the common operator error,
    //    a port in use or an address that is not local, and is returned as is.
    let listener = TcpListener::bind(settings.listen).await?;

    // 2. Serve until `shutdown_signal` resolves, then finish the in-flight
    //    requests and return, so a SIGTERM from an orchestrator drains the
    //    connections instead of cutting them.
    axum::serve(listener, router())
        .with_graceful_shutdown(shutdown_signal())
        .await
}

/// Resolves when the process is asked to stop: SIGINT or SIGTERM on Unix,
/// Ctrl-C on Windows.
async fn shutdown_signal() {
    // SIGTERM is what an orchestrator sends and SIGINT what a terminal sends;
    // either one stops the server.
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut terminate = signal(SignalKind::terminate()).expect("the SIGTERM handler installs");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = terminate.recv() => {}
        }
    }

    // Windows has no SIGTERM; Ctrl-C is the console signal tokio exposes on
    // every platform.
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .expect("the Ctrl-C handler installs");
    }
}
