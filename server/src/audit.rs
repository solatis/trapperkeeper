//! The audit trail: the middleware that records every changing request under
//! `/api/v1` once the handler has answered, the identity cell that the
//! extractors and handlers fill with the caller and the tenant acted on, and
//! the admin's read route (ADR-0031).

use std::convert::Infallible;
use std::future::ready;
use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};

use axum::Json;
use axum::extract::{
    ConnectInfo, FromRequestParts, MatchedPath, Path, Query, RawPathParams, Request, State,
};
use axum::http::Method;
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use utoipa::{IntoParams, ToSchema};

use crate::auth::role::{self, AtLeast};
use crate::http::{API_PREFIX, AppState};
use crate::scope;
use crate::store::audit::{self, Record, Row};
use crate::store::users;

/// Who a request turned out to be from, and which tenant it acted on.
/// `Caller`, `AtLeast` and the refresh handler set the caller with its user,
/// and the login handler sets a tenant with no user. `scope::tenant` and the
/// mint handler set the tenant acted on. The first setter of each wins.
#[derive(Clone, Default)]
pub struct Identity {
    /// The caller's tenant, and its user id when the setter gave one.
    caller: Arc<OnceLock<(String, Option<String>)>>,
    /// The tenant a tenant-scoped route resolved from its path.
    scope: Arc<OnceLock<String>>,
}

impl Identity {
    /// Records the caller's tenant, and the user id when one is given, on the
    /// first call. A later call changes nothing.
    pub fn set_caller(&self, tenant_id: &str, user_id: Option<&str>) {
        let _ = self
            .caller
            .set((tenant_id.to_owned(), user_id.map(str::to_owned)));
    }

    /// Records the tenant a tenant-scoped route resolved from its path, once.
    pub fn set_scope(&self, tenant_id: &str) {
        let _ = self.scope.set(tenant_id.to_owned());
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Identity {
    type Rejection = Infallible;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl Future<Output = Result<Self, Infallible>> + Send {
        // When `record` put no cell in, as for a GET, the extractor returns a
        // new cell that nothing reads, so the trail never fails a handler.
        ready(Ok(parts
            .extensions
            .get::<Self>()
            .cloned()
            .unwrap_or_default()))
    }
}

/// Runs the request. For a non-GET request that matched a route under
/// `/api/v1` it then appends one audit row. A failed append is logged at
/// error, and the handler's response is returned unchanged.
pub async fn record(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    matched: Option<MatchedPath>,
    request: Request,
    next: Next,
) -> Response {
    // The middleware runs the handler and then writes one audit row for a
    // changing request under `/api/v1`.
    //
    // 1. We record only a matched route under `/api/v1` with a method other
    //    than GET. The prefix keeps out the probes and `/openapi.json`.
    // 2. We read the method, X-Forwarded-For and the path parameters, never a
    //    body or a query string. The handler takes the request in step 4.
    // 3. We put an empty identity cell into the request. Its clones share it,
    //    so what extractors and handlers set in step 4 is visible afterwards.
    // 4. We run the handler, which commits its own transaction.
    // 5. We build the row. Its tenant is the one the route resolved, else the
    //    caller's, so a tenant's admins see what the superadmin did there.
    // 6. We append the row in a transaction of its own and only log a
    //    failure, because the handler's change is committed and a 500 would
    //    make the client retry it.

    // 1. decide whether it is recorded
    let route = matched
        .as_ref()
        .map(MatchedPath::as_str)
        // As ADR-0031 sets, a HEAD or an OPTIONS request leaves a row too.
        .filter(|route| route.starts_with(API_PREFIX) && request.method() != Method::GET);
    let Some(route) = route else {
        return next.run(request).await;
    };
    // 2. read what the row needs
    let method = request.method().to_string();
    let forwarded = request
        .headers()
        .get("x-forwarded-for")
        // A value that is not visible ASCII counts as absent.
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_owned());
    // As a middleware argument, `RawPathParams` would answer the request when
    // it fails. Here it fails only on a parameter that is not UTF-8, which
    // `.ok()` records as `{}`.
    let (mut parts, body) = request.into_parts();
    let params = RawPathParams::from_request_parts(&mut parts, &())
        .await
        .ok();
    let path_params = params_json(params.as_ref());

    // 3. put an empty identity in
    let identity = Identity::default();
    parts.extensions.insert(identity.clone());
    let request = Request::from_parts(parts, body);

    // 4. run the handler
    let response = next.run(request).await;

    // 5. build the row
    let caller = identity.caller.get();
    let tenant_id = identity
        .scope
        .get()
        .or(caller.map(|(tenant_id, _)| tenant_id));
    let user_id = caller.and_then(|(_, user_id)| user_id.as_deref());
    // The port names one TCP connection, and the row records the host.
    let peer_address = peer.ip().to_string();
    // One proxy is assumed, so a chain such as `a, b` is stored whole
    // (ADR-0031).
    let client_address = forwarded.as_deref().unwrap_or(&peer_address);
    let record = Record {
        tenant_id: tenant_id.map(String::as_str),
        user_id,
        method: &method,
        route,
        path_params: &path_params,
        status: response.status().as_u16(),
        peer_address: &peer_address,
        client_address,
    };

    // 6. append the row
    let appended = async {
        let mut tx = state.store.begin().await?;
        audit::append(&mut tx, record).await?;
        tx.commit().await
    }
    .await;
    if let Err(error) = appended {
        tracing::error!(%error, route, "the audit row was not written");
    }
    response
}

/// The path parameters as one JSON object of strings with the keys sorted,
/// and `{}` when there are none or the extraction failed (`None`).
fn params_json(params: Option<&RawPathParams>) -> String {
    // `serde_json`'s `Map` is a `BTreeMap`, which sorts the keys, because no
    // crate of the workspace enables its `preserve_order` feature.
    let object: Map<String, Value> = params
        .into_iter()
        .flat_map(RawPathParams::iter)
        .map(|(name, value)| (name.to_owned(), Value::String(value.to_owned())))
        .collect();
    Value::Object(object).to_string()
}

/// A row of the trail as the API shows it.
#[derive(Serialize, ToSchema)]
pub struct AuditView {
    /// The row's id, which is the `before` cursor of the next page.
    id: String,
    /// Epoch microseconds in UTC.
    recorded_at: i64,
    /// The name of the user who made the request. Null for a login and for a
    /// request refused before authentication.
    user: Option<String>,
    /// The user's own tenant, which is `$tk` when the superadmin acted on
    /// this tenant. It is null whenever `user` is null.
    user_tenant: Option<String>,
    method: String,
    /// The matched route template.
    route: String,
    /// Each parameter of the route template with its value from the path.
    path_params: Value,
    status: i32,
    /// The IP address of the TCP peer, without its port.
    peer_address: String,
    /// The X-Forwarded-For header when the request had one, and the peer
    /// address otherwise.
    client_address: String,
}

/// `row` as the API shows it.
fn view(row: Row) -> AuditView {
    AuditView {
        id: row.id,
        recorded_at: row.recorded_at,
        user: row.user_name,
        user_tenant: row.user_tenant,
        method: row.method,
        route: row.route,
        // Only `params_json` writes this column, so the value always parses.
        path_params: serde_json::from_str(&row.path_params).unwrap_or(Value::Null),
        status: row.status,
        peer_address: row.peer_address,
        client_address: row.client_address,
    }
}

/// The page asked for, and the user to narrow it to.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct Page {
    /// The name of a user whose rows alone are listed. It matches in any case
    /// and a deleted user too, and an unknown name gives an empty page.
    user: Option<String>,
    /// The id of the last row seen. The page holds the older rows.
    before: Option<String>,
    /// How many rows at most: 100 by default, 1000 at most.
    limit: Option<u32>,
}

/// Lists the tenant's trail, newest first. Needs admin of that tenant, or
/// superadmin.
#[utoipa::path(
    get,
    path = "/tenants/{tenant}/audit",
    params(("tenant" = String, Path, description = "the tenant's name"), Page),
    security(("bearer" = [])),
    responses(
        (status = OK, description = "the rows, newest first", body = Vec<AuditView>),
        (status = UNAUTHORIZED, description = "no valid access token", body = String),
        (status = FORBIDDEN, description = "the caller is below admin, or of another tenant", body = String),
        (status = NOT_FOUND, description = "no such tenant", body = String)
    )
)]
pub async fn list(
    caller: AtLeast<role::Admin>,
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Query(page): Query<Page>,
) -> Response {
    // A client that wants more rows pages with `before`. Both values are
    // choices that no measurement backs.
    let limit = page.limit.unwrap_or(100).min(1000);
    let listed = async {
        let mut tx = state.store.begin().await?;
        let tenant = scope::tenant(&mut tx, &caller.user, &caller.identity, &tenant).await?;
        let user_id = match &page.user {
            Some(name) => {
                // The store keeps user names lowercase (ADR-0020).
                match users::find_id_any(&mut tx, &tenant.id, &name.to_lowercase()).await? {
                    Some(user_id) => Some(user_id),
                    None => return Ok(Vec::new()),
                }
            }
            None => None,
        };
        let rows = audit::page(
            &mut tx,
            &tenant.id,
            user_id.as_deref(),
            page.before.as_deref(),
            limit,
        )
        .await?;
        // The block's error type is `scope::Refused`, so `?` turns a store
        // error into a 500.
        Ok::<_, scope::Refused>(rows)
    }
    .await;
    match listed {
        Ok(rows) => Json(rows.into_iter().map(view).collect::<Vec<_>>()).into_response(),
        Err(refused) => refused.into_response(),
    }
}
