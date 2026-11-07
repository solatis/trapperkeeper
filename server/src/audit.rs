//! The audit trail: the middleware that records every changing request under
//! `/api/v1` once the handler has answered, and the identity cell the handler
//! side fills with the caller and the tenant acted on
//! (doc/adr/0031-audit-trail.md).

use std::convert::Infallible;
use std::future::ready;
use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};

use axum::extract::{ConnectInfo, FromRequestParts, MatchedPath, RawPathParams, Request, State};
use axum::http::Method;
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;
use serde_json::{Map, Value};

use crate::http::{API_PREFIX, AppState};
use crate::store::audit::{self, Record};

/// Who a request turned out to be from, and which tenant it acted on, filled
/// by the handler side once it knows.
#[derive(Clone, Default)]
pub struct Identity {
    /// The caller's tenant, and its user when one was authenticated.
    caller: Arc<OnceLock<(String, Option<String>)>>,
    /// The tenant a tenant-scoped route resolved from its path.
    scope: Arc<OnceLock<String>>,
}

impl Identity {
    /// Records the caller's tenant and, when a user was authenticated, its
    /// id, once; a second call changes nothing.
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
        // An unfilled cell of its own when no middleware placed one, so a
        // handler never fails over the trail.
        ready(Ok(parts
            .extensions
            .get::<Self>()
            .cloned()
            .unwrap_or_default()))
    }
}

/// The middleware: runs the request, then appends one row for a non-GET
/// request that matched a route under `/api/v1`. A failed append is logged at
/// error and the response stands.
pub async fn record(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    matched: Option<MatchedPath>,
    request: Request,
    next: Next,
) -> Response {
    // 1. decide whether this request is recorded
    // 2. put an empty identity into the request's extensions
    // 3. run the handler
    // 4. read the identity and build the row
    // 5. append the row in a transaction of its own
    // 6. return the handler's response

    // 1. A GET changes nothing, and a request no route matched has no
    //    template, so neither is recorded; the prefix check keeps the probes
    //    and the document out.
    let route = matched
        .as_ref()
        .map(MatchedPath::as_str)
        .filter(|route| route.starts_with(API_PREFIX) && request.method() != Method::GET);
    let Some(route) = route else {
        return next.run(request).await;
    };
    let method = request.method().to_string();
    let forwarded = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_owned());
    // Read from the parts by hand, because the extractor has no optional
    // form and rejects a route without parameters.
    let (mut parts, body) = request.into_parts();
    let params = RawPathParams::from_request_parts(&mut parts, &())
        .await
        .ok();
    let path_params = params_json(params.as_ref());

    // 2. The cell is shared with the handler side by Arc, so the Caller
    //    extractor and the login handler fill it after this function has
    //    moved the request on.
    let identity = Identity::default();
    parts.extensions.insert(identity.clone());
    let request = Request::from_parts(parts, body);

    // 3.
    let response = next.run(request).await;

    // 4. The row's tenant is the one the route resolved from its path when it
    //    did, and the caller's otherwise, so a tenant's admins see what the
    //    superadmin did to their tenant.
    let caller = identity.caller.get();
    let tenant_id = identity
        .scope
        .get()
        .or(caller.map(|(tenant_id, _)| tenant_id));
    let user_id = caller.and_then(|(_, user_id)| user_id.as_deref());
    // The peer's port is dropped, because it names a connection, not a host.
    let peer_address = peer.ip().to_string();
    // X-Forwarded-For as received when present, else the peer; one proxy is
    // assumed, and chains of proxies are out of scope.
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

    // 5. A failed append is logged at error and does not change the
    //    response, because the handler's change is already committed and a
    //    500 would mislead the client into retrying it.
    let appended = async {
        let mut tx = state.store.begin().await?;
        audit::append(&mut tx, record).await?;
        tx.commit().await
    }
    .await;
    if let Err(error) = appended {
        tracing::error!(%error, route, "the audit row was not written");
    }

    // 6.
    response
}

/// The path parameters as one JSON object, or `{}` when the route has none.
/// The keys come out sorted, as `serde_json`'s map keeps them; the query
/// string and the body are never read.
fn params_json(params: Option<&RawPathParams>) -> String {
    let object: Map<String, Value> = params
        .into_iter()
        .flat_map(RawPathParams::iter)
        .map(|(name, value)| (name.to_owned(), Value::String(value.to_owned())))
        .collect();
    Value::Object(object).to_string()
}
