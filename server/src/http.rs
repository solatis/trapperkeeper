//! The HTTP server: the router, the operational endpoints, the management
//! routes under `/api/v1`, the API document generated from the handlers, and
//! how the server is served and stopped. The probe paths follow the Kubernetes
//! convention, and the document sits at the conventional file name.

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{FromRef, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router, middleware};
use tokio::net::TcpListener;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::audit;
use crate::auth::login;
use crate::auth::token::Keyring;
use crate::config::Settings;
use crate::sessions;
use crate::store::{self, Store};
use crate::tenants;
use crate::users;

/// The prefix every management route sits under (ADR-0026).
pub const API_PREFIX: &str = "/api/v1";

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

/// The API document. The package manifest supplies its title, version and
/// license, and the router adds each route as its handler is registered.
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
                        // The access tokens that the bearer scheme carries
                        // are HS256 JWTs (ADR-0025).
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
    // The body is a plain literal and no JSON object, because an orchestrator
    // reads only the status code and a person reads the body, so no shape of
    // the body is worth keeping.
    "ok"
}

/// Readiness: the instance may receive traffic. It answers 200 with `ok`
/// while the store answers a trivial query, and 503 with `unavailable` while
/// it does not.
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
        // The 503 body is a plain literal for the same reason as the liveness
        // body.
        (StatusCode::SERVICE_UNAVAILABLE, "unavailable")
    }
}

/// Builds the router with the management routes under `/api/v1` and the
/// probes at the root, the API document that describes them, served at
/// `/openapi.json`, the audit trail and the request log. The handlers share
/// `state`.
pub fn router(state: AppState) -> Router {
    // `router` builds the router through utoipa-axum, so the API document is
    // generated as the handlers are registered (ADR-0016). It splits the
    // document off before it adds the route that serves it, and it wraps the
    // whole router in the audit and trace layers at the end.
    //
    // 1. We register each management handler on an `OpenApiRouter` through
    //    `routes!`, without the prefix. `routes!` registers a handler and its
    //    description in one call, and a handler added with `.route` would be
    //    missing from the document.
    // 2. We start the document from `ApiDoc`, nest the management routes under
    //    `API_PREFIX` and register the two probes at the root. The management
    //    routes live under the versioned prefix, and the probes stay at the
    //    root, where orchestrators look for them (ADR-0026).
    // 3. We split the router from the finished document. The route that the
    //    next step adds serves the document that the split returns, so the
    //    split comes first.
    // 4. We build the audit layer, add the route that serves the document at
    //    `/openapi.json`, and give the handlers the state. The document stays
    //    at the root, where tools look for it (ADR-0026), and its route goes
    //    on the plain axum router, so the document does not list it.
    // 5. We wrap the whole router in the audit layer and then in the trace
    //    layer, so the trace layer is the outer one. One middleware over every
    //    route cannot be forgotten, and the trace span around it also covers
    //    the audit write and the document route.

    // 1. register the management routes
    let management = OpenApiRouter::new()
        .routes(routes!(login::login))
        .routes(routes!(login::refresh))
        .routes(routes!(login::logout))
        .routes(routes!(login::change_password))
        .routes(routes!(login::me))
        .routes(routes!(tenants::list, tenants::create))
        .routes(routes!(users::list, users::create))
        .routes(routes!(users::delete))
        .routes(routes!(users::change_role))
        .routes(routes!(users::reset_password))
        .routes(routes!(sessions::mint))
        .routes(routes!(audit::list));

    // 2. nest the management routes and add the probes
    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest(API_PREFIX, management)
        .routes(routes!(healthz))
        .routes(routes!(readyz))
        // 3. split off the document
        .split_for_parts();

    // 4. add the document route and the state
    // The audit middleware takes the state as well, so it is built from a
    // clone before `with_state` moves the state.
    let audit = middleware::from_fn_with_state(state.clone(), audit::record);
    let router = router
        .route("/openapi.json", get(move || async move { Json(api) }))
        .with_state(state);

    // 5. wrap the router in the audit and trace layers
    router.layer(audit).layer(
        // The request-start and end-of-stream events of tower-http stay at
        // DEBUG, so they add no lines at the default level.
        TraceLayer::new_for_http()
            // tower-http writes the span and the response event at DEBUG,
            // and these two calls raise them to INFO, so each request is one
            // INFO line with method, URI, status and latency.
            .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
            .on_response(DefaultOnResponse::new().level(Level::INFO)),
    )
}

/// Serves the router with `state` on the configured address until a shutdown
/// signal, then finishes the in-flight requests and returns.
///
/// # Errors
///
/// Returns the I/O error when installing a signal handler, binding, reading
/// the bound address or serving fails, and prints nothing, so the caller
/// reports it.
pub async fn serve(settings: &Settings, state: AppState) -> io::Result<()> {
    // `serve` installs the signal handlers, binds the listen address, serves
    // the router until a signal arrives and drains the in-flight requests.
    // The first three steps can fail and return their I/O error, and the bind
    // and the clean stop each write an info line.
    //
    // 1. We install the handlers for the signals that stop the server. This
    //    comes before the bind, so a signal sent once the "listening" line
    //    appears stops the server through the drain instead of killing it.
    // 2. We bind the configured listen address and log the bound address at
    //    info, which tells an operator that the server accepts connections.
    // 3. We serve the router until the shutdown future resolves, then finish
    //    the in-flight requests, so a SIGTERM from an orchestrator drains the
    //    connections instead of cutting them.
    // 4. We log "stopped" once the drain completes, so the log tells a clean
    //    stop from a crash.

    // 1. install the signal handlers
    let shutdown = shutdown_signal()?;

    // 2. bind and say so
    // A failed bind is the common operator error, a port in use or an address
    // that is not local.
    let listener = TcpListener::bind(settings.listen).await?;
    tracing::info!(address = %listener.local_addr()?, "listening");

    // 3. serve and drain
    axum::serve(
        listener,
        // The service carries connect info, because the audit middleware
        // reads the peer's address.
        router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await?;

    // 4. say that the drain completed
    tracing::info!("stopped");
    Ok(())
}

/// Installs the handlers for the signals that stop the server, SIGINT or
/// SIGTERM on Unix and Ctrl-C on Windows, and returns a future that resolves
/// when one arrives. Fails when a handler cannot be installed.
fn shutdown_signal() -> io::Result<impl Future<Output = ()>> {
    // An orchestrator sends SIGTERM and a terminal sends SIGINT, so the Unix
    // block waits for both.
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
        // The future logs the signal at INFO, so the log shows that a signal
        // caused the stop that follows and no crash.
        tracing::info!("shutdown signal received");
    })
}
