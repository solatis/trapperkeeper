//! The HTTP server: the router, the operational endpoints, the management
//! routes under `/api/v1`, the API document generated from the handlers, and
//! how the server is served and stopped. The probe paths follow the Kubernetes
//! convention, and the document sits at the conventional file name.

use std::io;
use std::sync::Arc;

use axum::extract::{FromRef, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use tokio::net::TcpListener;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::auth::login;
use crate::auth::token::Keyring;
use crate::config::Settings;
use crate::store::{self, Store};

/// What every handler can reach: the store and the keyring.
#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub keyring: Arc<Keyring>,
}

// Written by hand, so axum's macros feature stays off.
impl FromRef<AppState> for Store {
    fn from_ref(state: &AppState) -> Self {
        state.store.clone()
    }
}

/// The API document. Title, version and license come from the package
/// manifest; the routes are added by the router as the handlers are
/// registered, and each handler's doc comment becomes its operation summary.
#[derive(OpenApi)]
#[openapi(modifiers(&BearerScheme))]
struct ApiDoc;

/// Adds the bearer scheme the management routes name in `security`.
struct BearerScheme;

impl Modify for BearerScheme {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        openapi
            .components
            .get_or_insert_with(Default::default)
            .add_security_scheme(
                "bearer",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
    }
}

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

/// Builds the router with the management routes under `/api/v1` and the
/// probes at the root, the API document that describes them, served at
/// `/openapi.json`, and the request log. The handlers share `state`.
pub fn router(state: AppState) -> Router {
    // 1. register the management routes, nested under /api/v1
    // 2. register the probes at the root
    // 3. split the router from the finished document
    // 4. add the route that serves the document, and give the handlers the state
    // 5. wrap the whole router with the trace layer
    //
    // The document must be split off before the route that serves it is
    // added, or it would describe itself, and the trace layer must wrap every
    // route including that one.

    // 1. Each annotated handler enters through `routes!`, so the handler and
    //    its description enter together and neither can be forgotten. The
    //    prefix is the versioned namespace of doc/adr/0026.
    let management = OpenApiRouter::new()
        .routes(routes!(login::login))
        .routes(routes!(login::refresh))
        .routes(routes!(login::me));

    // 2. The probes and the document stay at the root, where orchestrators
    //    and tools look for them.
    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/api/v1", management)
        .routes(routes!(healthz))
        .routes(routes!(readyz))
        // 3.
        .split_for_parts();

    // 4. The document route is not in the document itself.
    let router = router
        .route("/openapi.json", get(move || async move { Json(api) }))
        .with_state(state);

    // 5. The trace layer's span and response event are raised to INFO, so
    //    each request is one info line with method, URI, status and latency
    //    at the default level. tower-http emits both at DEBUG, where its
    //    "started processing request" and "end of stream" events stay,
    //    because one line per request is the point.
    router.layer(
        TraceLayer::new_for_http()
            .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
            .on_response(DefaultOnResponse::new().level(Level::INFO)),
    )
}

/// Serves the router with `state` on the configured address until a shutdown
/// signal, then finishes the in-flight requests and returns.
///
/// # Errors
///
/// An I/O error binding or serving is returned, not printed.
pub async fn serve(settings: &Settings, state: AppState) -> io::Result<()> {
    // Listen for signals, bind, serve, stop: a sequence with its own failure
    // at each step and a log line at the bind and the stop.
    //
    // 1. Install the signal handlers before the bind, so a signal sent once
    //    the "listening" line appears stops the server through the drain
    //    instead of killing it.
    let shutdown = shutdown_signal()?;

    // 2. Bind the listen address and say so at info; it is the line a test or
    //    an operator waits for. Failure here is the common operator error, a
    //    port in use or an address that is not local, and is returned as is.
    let listener = TcpListener::bind(settings.listen).await?;
    tracing::info!(address = %listener.local_addr()?, "listening");

    // 3. Serve until a signal arrives, then finish the in-flight requests, so
    //    a SIGTERM from an orchestrator drains the connections instead of
    //    cutting them.
    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown)
        .await?;

    // 4. Say that the drain completed, so the log can tell a clean stop from a
    //    crash.
    tracing::info!("stopped");
    Ok(())
}

/// Installs the handlers for the signals that stop the server, SIGINT or
/// SIGTERM on Unix and Ctrl-C on Windows, and returns a future that resolves
/// when one arrives. Fails when a handler cannot be installed.
fn shutdown_signal() -> io::Result<impl Future<Output = ()>> {
    // SIGTERM is what an orchestrator sends and SIGINT what a terminal sends;
    // either one stops the server.
    #[cfg(unix)]
    let received = {
        use tokio::signal::unix::{SignalKind, signal};

        let mut terminate = signal(SignalKind::terminate())?;
        let mut interrupt = signal(SignalKind::interrupt())?;
        async move {
            tokio::select! {
                _ = terminate.recv() => {}
                _ = interrupt.recv() => {}
            }
        }
    };

    // Windows has no SIGTERM, so Ctrl-C is the console signal that stops the
    // server.
    #[cfg(not(unix))]
    let received = {
        let mut ctrl_c = tokio::signal::windows::ctrl_c()?;
        async move {
            ctrl_c.recv().await;
        }
    };

    Ok(async move {
        received.await;
        // Said at info, so the stop that follows is attributable to the
        // signal rather than to a crash.
        tracing::info!("shutdown signal received");
    })
}
