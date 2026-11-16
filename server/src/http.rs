//! The HTTP server: the router, the operational endpoints, the management
//! routes under `/api/v1`, the API document generated from the handlers, and
//! how the server is served and stopped. The probe paths follow the Kubernetes
//! convention, and the document sits at the conventional file name.

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{FromRef, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Extension, Json, Router, middleware};
use tokio::net::TcpListener;
use tokio::time;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::audit;
use crate::auth::login;
use crate::auth::token::Keyring;
use crate::config::{READINESS_TIMEOUT_SECONDS, Settings};
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

// `readyz` takes the store alone, and the other handlers take the whole
// `AppState`. The impl is written by hand so that axum's macros feature,
// which would derive it, stays off.
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
    // reads only the status code and a person reads the body.
    "ok"
}

/// How long `/readyz` waits for the store. `serve` adds it to the router from
/// the settings, and a router without it waits `READINESS_TIMEOUT_SECONDS`.
#[derive(Clone, Copy)]
pub struct ReadinessTimeout(pub Duration);

/// Readiness: the instance may receive traffic. It answers 200 with `ok`
/// while the store answers a trivial query in time, and 503 with
/// `unavailable` while it does not.
#[utoipa::path(
    get,
    path = "/readyz",
    responses(
        (status = OK, description = "the instance accepts traffic", body = String),
        (status = SERVICE_UNAVAILABLE, description = "the store does not answer in time", body = String)
    )
)]
async fn readyz(
    State(store): State<Store>,
    timeout: Option<Extension<ReadinessTimeout>>,
) -> (StatusCode, &'static str) {
    let limit = timeout.map_or(
        Duration::from_secs(READINESS_TIMEOUT_SECONDS),
        |Extension(ReadinessTimeout(limit))| limit,
    );
    readiness(store::ping(&store), limit).await
}

/// 200 with `ok` when `ping` gives true within `limit`, and 503 with
/// `unavailable` otherwise.
async fn readiness(
    ping: impl Future<Output = bool>,
    limit: Duration,
) -> (StatusCode, &'static str) {
    if time::timeout(limit, ping).await == Ok(true) {
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
    // utoipa-axum generates the API document as the handlers are registered
    // (ADR-0016).
    //
    // 1. We register the management handlers through `routes!`, which adds
    //    each one to the document too.
    // 2. We nest them under `API_PREFIX` and add the probes at the root, where
    //    orchestrators look for them (ADR-0026).
    // 3. We split off the document, which the next step serves.
    // 4. We add the route of the document at the root, where tools look for
    //    it (ADR-0026), and give the handlers the state. The route bypasses
    //    `routes!`, so the document does not list itself.
    // 5. We wrap the router in the audit layer and then the trace layer, so
    //    the trace span also covers the audit write.

    // 1. register the management routes
    // Each `routes!` takes the handlers of one path, one per method.
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
    // The audit layer takes a clone of the state before `with_state` moves it.
    let audit = middleware::from_fn_with_state(state.clone(), audit::record);
    let router = router
        .route("/openapi.json", get(move || async move { Json(api) }))
        .with_state(state);

    // 5. wrap in the audit and trace layers
    router.layer(audit).layer(
        TraceLayer::new_for_http()
            // Raising the span and the response event from tower-http's DEBUG
            // makes each request one INFO line at the default level.
            .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
            .on_response(DefaultOnResponse::new().level(Level::INFO)),
    )
}

/// Serves the router with `state` on the configured address until a shutdown
/// signal, then finishes the in-flight requests and returns.
///
/// # Errors
///
/// Returns the I/O error of a signal handler, the bind, reading the bound
/// address or serving, and does not print it, so the caller reports it.
pub async fn serve(settings: &Settings, state: AppState) -> io::Result<()> {
    // `serve` runs the router until a signal and stops it cleanly.
    //
    // 1. We install the signal handlers before the bind. A signal kills the
    //    process until they are installed, so once the "listening" line
    //    shows, a signal stops the server through the drain.
    // 2. We bind the listen address and log it.
    // 3. We serve, with the readiness timeout of the settings, until a signal
    //    and then drain the in-flight requests, so a SIGTERM does not cut
    //    connections.
    // 4. We log "stopped", so the log tells a clean stop from a crash.

    // 1. install the signal handlers
    let shutdown = shutdown_signal()?;

    // 2. bind and say so
    let listener = TcpListener::bind(settings.listen).await?;
    // The bound address shows the port that the system picked for port 0.
    tracing::info!(address = %listener.local_addr()?, "listening");

    // 3. serve and drain
    let timeout = Duration::from_secs(settings.readiness_timeout_seconds);
    let router = router(state).layer(Extension(ReadinessTimeout(timeout)));
    axum::serve(
        listener,
        // The audit middleware reads the peer's address from the connect info.
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await?;

    // 4. log the stop
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
        // The log then shows that a signal caused the stop that follows.
        tracing::info!("shutdown signal received");
    })
}

#[cfg(test)]
mod tests {
    use std::future;
    use std::time::Duration;

    use axum::http::StatusCode;

    use super::readiness;

    #[tokio::test]
    async fn a_ping_that_does_not_finish_in_time_is_unavailable() {
        // A pending ping never finishes, so the answer is the timeout's.
        let late = readiness(future::pending(), Duration::ZERO).await;
        assert_eq!(late, (StatusCode::SERVICE_UNAVAILABLE, "unavailable"));
        let ready = readiness(future::ready(true), Duration::from_secs(1)).await;
        assert_eq!(ready, (StatusCode::OK, "ok"));
    }
}
