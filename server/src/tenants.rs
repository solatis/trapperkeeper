//! The superadmin's routes for tenants: listing them and creating one with
//! its first admin.

use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

use crate::auth::login::database_error;
use crate::auth::role::{self, AtLeast};
use crate::http::AppState;
use crate::store::tenants;

/// A tenant as the list shows it.
#[derive(Serialize, ToSchema)]
pub struct TenantView {
    name: String,
    title: String,
}

/// Lists the live tenants. Needs superadmin.
#[utoipa::path(
    get,
    path = "/tenants",
    security(("bearer" = [])),
    responses(
        (status = OK, description = "the tenants, by name", body = Vec<TenantView>),
        (status = UNAUTHORIZED, description = "no valid access token", body = String),
        (status = FORBIDDEN, description = "the caller is not the superadmin", body = String)
    )
)]
pub async fn list(_caller: AtLeast<role::Superadmin>, State(state): State<AppState>) -> Response {
    let listed = async {
        let mut tx = state.store.begin().await?;
        tenants::list(&mut tx).await
    }
    .await;
    match listed {
        Ok(listed) => Json(
            listed
                .into_iter()
                .map(|t| TenantView {
                    name: t.name,
                    title: t.title,
                })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => database_error(&error),
    }
}
