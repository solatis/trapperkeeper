//! The HTTP server: the router, the operational endpoints, the API document
//! generated from the handlers, and how the server is served and stopped.

use std::io;

use axum::routing::get;
use axum::{Json, Router};
use tokio::net::TcpListener;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::config::Settings;

/// The API document. Title, version and license come from the package
/// manifest; the routes are added by the router as the handlers are
/// registered, and each handler's doc comment becomes its operation summary.
#[derive(OpenApi)]
struct ApiDoc;

/// Liveness: the process is running and can answer. Always `ok`.
#[utoipa::path(
    get,
    path = "/healthz",
    responses((status = OK, description = "the process is alive", body = String))
)]
async fn healthz() -> &'static str {
    "ok"
}

/// Readiness: the instance may receive traffic. `ok` as soon as the listener
/// is up; the store issue adds the store probe.
#[utoipa::path(
    get,
    path = "/readyz",
    responses((status = OK, description = "the instance accepts traffic", body = String))
)]
async fn readyz() -> &'static str {
    // No probe yet, by decision: there is no store in this unit to ask.
    // TODO(store issue): probe the store here.
    "ok"
}

/// Builds the router with every route and the API document that describes
/// them, served at `/openapi.json`.
fn router() -> Router {
    // The document must be split off before the route that serves it is
    // added, or it would describe itself; the order is the point.
    //
    // 1. Register each annotated handler through `routes!`, so the handler and
    //    its description enter together and neither can be forgotten.
    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(healthz))
        .routes(routes!(readyz))
        // 2. Split the router from the finished document.
        .split_for_parts();

    // 3. Add the route that serves the document as JSON; it is not in the
    //    document itself.
    router.route("/openapi.json", get(move || async move { Json(api) }))
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
