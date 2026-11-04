//! The HTTP server: the router, the operational endpoints, the API document
//! generated from the handlers, and how the server is served and stopped. The
//! probe paths follow the Kubernetes convention, and the document sits at the
//! conventional file name.

use std::io;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use tokio::net::TcpListener;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::config::Settings;
use crate::store::{self, Store};

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
    // The literal rather than a JSON object: an orchestrator reads the status
    // code and a person reads the body, so there is no shape worth keeping.
    "ok"
}

/// Readiness: the instance may receive traffic. `ok` while the store answers,
/// 503 while it does not.
#[utoipa::path(
    get,
    path = "/readyz",
    responses(
        (status = OK, description = "the instance accepts traffic", body = String),
        (status = SERVICE_UNAVAILABLE, description = "the store does not answer", body = String)
    )
)]
async fn readyz(State(store): State<Store>) -> (StatusCode, &'static str) {
    if store::ping(&store).await {
        (StatusCode::OK, "ok")
    } else {
        // A literal for the same reason as the liveness body.
        (StatusCode::SERVICE_UNAVAILABLE, "unavailable")
    }
}

/// Builds the router with every route, the API document that describes them,
/// served at `/openapi.json`, and the request log. The handlers share `store`.
pub fn router(store: Store) -> Router {
    // The document must be split off before the route that serves it is
    // added, or it would describe itself, and the trace layer must wrap every
    // route including that one.
    //
    // 1. Register each annotated handler through `routes!`, so the handler and
    //    its description enter together and neither can be forgotten.
    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(healthz))
        .routes(routes!(readyz))
        // 2. Split the router from the finished document.
        .split_for_parts();

    // 3. Add the route that serves the document as JSON; it is not in the
    //    document itself. Then give the handlers the store.
    let router = router
        .route("/openapi.json", get(move || async move { Json(api) }))
        .with_state(store);

    // 4. Wrap the whole router with the trace layer. Its span and its response
    //    event are raised to INFO, so each request is one info line with
    //    method, URI, status and latency at the default level. tower-http
    //    emits both at DEBUG, where its "started processing request" and "end
    //    of stream" events stay, because one line per request is the point.
    router.layer(
        TraceLayer::new_for_http()
            .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
            .on_response(DefaultOnResponse::new().level(Level::INFO)),
    )
}

/// Serves the router with `store` on the configured address until a shutdown
/// signal, then finishes the in-flight requests and returns.
///
/// # Errors
///
/// An I/O error binding or serving is returned, not printed.
pub async fn serve(settings: &Settings, store: Store) -> io::Result<()> {
    // Bind, serve, stop: a sequence with its own failure at each step and a
    // log line at each.
    //
    // 1. Bind the listen address and say so at info; it is the line a test or
    //    an operator waits for. Failure here is the common operator error, a
    //    port in use or an address that is not local, and is returned as is.
    let listener = TcpListener::bind(settings.listen).await?;
    tracing::info!(address = %listener.local_addr()?, "listening");

    // 2. Serve until `shutdown_signal` resolves, then finish the in-flight
    //    requests, so a SIGTERM from an orchestrator drains the connections
    //    instead of cutting them.
    axum::serve(listener, router(store))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // 3. Say that the drain completed, so the log can tell a clean stop from a
    //    crash.
    tracing::info!("stopped");
    Ok(())
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

    // Said at info, so the stop that follows is attributable to the signal
    // rather than to a crash.
    tracing::info!("shutdown signal received");
}
