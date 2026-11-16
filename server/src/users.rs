//! The routes for the users of a tenant, under
//! `/api/v1/tenants/{tenant}/users`. An admin of the tenant or the superadmin
//! may call every route, and any user of the tenant may delete itself. The
//! delete and the role change refuse with 409 a change that would leave the
//! tenant without a live admin.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::accounts::{self, NewAccount};
use crate::auth::caller::{Caller, Refusal};
use crate::auth::login::database_error;
use crate::auth::role::{self, AtLeast, Rank};
use crate::http::AppState;
use crate::names;
use crate::scope;
use crate::store::now_micros;
use crate::store::tenants::{self, ADMIN_ROLE};
use crate::store::users;
use crate::superadmin;

/// Why a user route refused a request or failed. It converts into the
/// response the caller gets.
enum Refused {
    /// `scope::tenant` refused the tenant in the path: 404 for an unknown
    /// tenant, 403 for one the caller may not act on, or 500.
    Scope(scope::Refused),
    /// The request's content breaks a rule: 400 with the rule.
    BadRequest(&'static str),
    /// The caller may not act on this user: 403.
    Forbidden,
    /// No live user of the tenant has the name: 404.
    NoSuchUser,
    /// A live user of the tenant has the name: 409.
    NameTaken,
    /// The change would leave the tenant without a live admin: 409.
    LastAdmin,
    /// The store failed, which is a 500.
    Store(Box<sqlx::Error>),
}

impl From<scope::Refused> for Refused {
    fn from(refused: scope::Refused) -> Self {
        Self::Scope(refused)
    }
}

impl From<sqlx::Error> for Refused {
    fn from(error: sqlx::Error) -> Self {
        Self::Store(Box::new(error))
    }
}

impl From<users::CreateError> for Refused {
    fn from(error: users::CreateError) -> Self {
        match error {
            users::CreateError::NameTaken => Self::NameTaken,
            users::CreateError::Database(error) => Self::Store(Box::new(error)),
        }
    }
}

impl IntoResponse for Refused {
    fn into_response(self) -> Response {
        match self {
            Self::Scope(refused) => refused.into_response(),
            Self::BadRequest(rule) => (StatusCode::BAD_REQUEST, rule).into_response(),
            Self::Forbidden => Refusal::Forbidden.into_response(),
            Self::NoSuchUser => (StatusCode::NOT_FOUND, "no such user").into_response(),
            Self::NameTaken => {
                (StatusCode::CONFLICT, "a user with this name exists").into_response()
            }
            Self::LastAdmin => (
                StatusCode::CONFLICT,
                "the tenant would be left without a live admin",
            )
                .into_response(),
            Self::Store(error) => database_error(&error),
        }
    }
}

/// A user as the list and the role change return it.
#[derive(Serialize, ToSchema)]
pub struct UserView {
    /// The tenant's internal UUID, which differs from the tenant name in the
    /// path.
    tenant_id: String,
    name: String,
    role: String,
    /// Whether the UI should ask for a new password at the next login.
    password_must_change: bool,
}

/// Lists the live users of the tenant, by name. Needs admin of that tenant,
/// or superadmin.
#[utoipa::path(
    get,
    path = "/tenants/{tenant}/users",
    params(("tenant" = String, Path, description = "the tenant's name")),
    security(("bearer" = [])),
    responses(
        (status = OK, description = "the users, by name", body = Vec<UserView>),
        (status = UNAUTHORIZED, description = "no valid access token", body = String),
        (status = FORBIDDEN, description = "the caller is below admin, or of another tenant", body = String),
        (status = NOT_FOUND, description = "no such tenant", body = String)
    )
)]
pub async fn list(
    caller: AtLeast<role::Admin>,
    State(state): State<AppState>,
    Path(tenant): Path<String>,
) -> Response {
    let listed = async {
        let mut tx = state.store.begin().await?;
        let tenant = scope::tenant(&mut tx, &caller.user, &caller.identity, &tenant).await?;
        Ok::<_, Refused>(users::list(&mut tx, &tenant.id).await?)
    }
    .await;
    match listed {
        Ok(listed) => Json(listed.into_iter().map(view).collect::<Vec<_>>()).into_response(),
        Err(refused) => refused.into_response(),
    }
}

fn view(user: users::User) -> UserView {
    UserView {
        tenant_id: user.tenant_id,
        name: user.name,
        role: user.role_name,
        password_must_change: user.password_must_change,
    }
}

/// Who to create: an email address and one of the tenant's roles.
#[derive(Deserialize, ToSchema)]
pub struct NewUserRequest {
    name: String,
    role: String,
}

/// What a create or a password reset returns: the user and its generated
/// password. The password reaches the caller only in this response.
#[derive(Serialize, ToSchema)]
pub struct CreatedUser {
    /// The tenant's internal UUID, which differs from the tenant name in the
    /// path.
    tenant_id: String,
    name: String,
    role: String,
    password: String,
}

/// Creates a user of the tenant with a generated password that must be
/// changed. Needs admin of that tenant, or superadmin. Answers 400 for a bad
/// name, for a role the tenant does not hold and for `superadmin`, and 409
/// when a live user of the tenant has the name.
#[utoipa::path(
    post,
    path = "/tenants/{tenant}/users",
    params(("tenant" = String, Path, description = "the tenant's name")),
    security(("bearer" = [])),
    request_body = NewUserRequest,
    responses(
        (status = CREATED, description = "the user and its one-time password", body = CreatedUser),
        (status = BAD_REQUEST, description = "the name breaks its rule, or the tenant does not hold the role", body = String),
        (status = UNAUTHORIZED, description = "no valid access token", body = String),
        (status = FORBIDDEN, description = "the caller is below admin, or of another tenant", body = String),
        (status = NOT_FOUND, description = "no such tenant", body = String),
        (status = CONFLICT, description = "a user of the tenant has the name", body = String)
    )
)]
pub async fn create(
    caller: AtLeast<role::Admin>,
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Json(request): Json<NewUserRequest>,
) -> Response {
    // The checks of the request come before the tenant lookup, because they
    // need no database. The checked name comes back lowercased, as the store
    // keeps it.
    let name = match names::user_name(&request.name) {
        Ok(name) => name,
        Err(refused) => return Refused::BadRequest(refused.0).into_response(),
    };
    // Only the system tenant `$tk` holds `superadmin` (ADR-0028). The role
    // check below would refuse it too, and this check names the reason.
    if request.role == superadmin::ROLE {
        return Refused::BadRequest("no route gives the role superadmin").into_response();
    }

    let created = async {
        let mut tx = state.store.begin().await?;
        let tenant = scope::tenant(&mut tx, &caller.user, &caller.identity, &tenant).await?;
        // A role the tenant does not hold would fail the insert with a 500.
        if !tenants::role_exists(&mut tx, &tenant.id, &request.role).await? {
            return Err(Refused::BadRequest("the tenant does not hold this role"));
        }
        let password = accounts::create(
            &mut tx,
            NewAccount {
                tenant_id: &tenant.id,
                name: &name,
                role_name: &request.role,
            },
        )
        .await?;
        tx.commit().await?;
        Ok((tenant.id, password))
    }
    .await;

    match created {
        Ok((tenant_id, password)) => (
            StatusCode::CREATED,
            Json(CreatedUser {
                tenant_id,
                name,
                role: request.role,
                password,
            }),
        )
            .into_response(),
        Err(refused) => refused.into_response(),
    }
}

/// Deletes the named user of the tenant. Needs admin of that tenant, the
/// superadmin, or the user itself. Answers 404 for an unknown name to an admin
/// or the superadmin and 403 to anyone else, and 409 for the tenant's last
/// live admin, also when that admin deletes itself.
#[utoipa::path(
    delete,
    path = "/tenants/{tenant}/users/{name}",
    params(
        ("tenant" = String, Path, description = "the tenant's name"),
        ("name" = String, Path, description = "the user's email address")
    ),
    security(("bearer" = [])),
    responses(
        (status = OK, description = "deleted", body = String),
        (status = UNAUTHORIZED, description = "no valid access token", body = String),
        (status = FORBIDDEN, description = "the caller is neither an admin of the tenant nor the user", body = String),
        (status = NOT_FOUND, description = "no such tenant or user", body = String),
        (status = CONFLICT, description = "the user is the tenant's last live admin", body = String)
    )
)]
pub async fn delete(
    caller: Caller,
    State(state): State<AppState>,
    Path((tenant, name)): Path<(String, String)>,
) -> Response {
    // The handler deletes the user when the caller may, and commits last, so
    // every refusal writes nothing. It takes `Caller` rather than
    // `AtLeast<Admin>`, because a user below admin may delete itself.
    //
    // 1. We resolve the tenant, which refuses another tenant's member, and
    //    find the target.
    // 2. We decide the permission. It needs the target's id, so it comes
    //    after the lookup, and a caller below admin gets 403 for any other
    //    name, so it cannot tell whether that name exists.
    // 3. We refuse to delete the tenant's last live admin, also when that
    //    admin is the caller.
    // 4. We mark the user deleted and commit. Its tokens need no revocation,
    //    because every request reads the user (ADR-0025).

    let deleted = async {
        // 1. resolve the tenant and the target
        let mut tx = state.store.begin().await?;
        let tenant = scope::tenant(&mut tx, &caller.user, &caller.identity, &tenant).await?;
        // The store keeps names lowercase (ADR-0020). A name that fails the
        // user-name rule matches no row, so no shape is checked.
        let found = users::find(&mut tx, &tenant.id, &name.to_lowercase()).await?;

        // 2. decide the permission
        // A role off the ladder ranks below admin.
        let deletes_anyone =
            Rank::of(&caller.user.role_name).is_some_and(|rank| rank >= Rank::Admin);
        let target = match found {
            Some(target) if deletes_anyone || target.id == caller.user.id => target,
            None if deletes_anyone => return Err(Refused::NoSuchUser),
            _ => return Err(Refused::Forbidden),
        };

        // 3. refuse the last admin
        // The count is a statement of its own, so two concurrent deletes on
        // Postgres or MariaDB can both pass it and leave no live admin.
        if target.role_name == ADMIN_ROLE && users::count_admins(&mut tx, &tenant.id).await? <= 1 {
            return Err(Refused::LastAdmin);
        }

        // 4. delete and commit
        // `users::delete` returns false when another request deleted the user
        // since the find.
        if !users::delete(&mut tx, &tenant.id, &target.id, now_micros()).await? {
            return Err(Refused::NoSuchUser);
        }
        tx.commit().await?;
        Ok(())
    }
    .await;
    match deleted {
        Ok(()) => (StatusCode::OK, "deleted").into_response(),
        Err(refused) => refused.into_response(),
    }
}

/// The role to give.
#[derive(Deserialize, ToSchema)]
pub struct RoleChange {
    role: String,
}

/// Gives the named user of the tenant one of the tenant's roles. Needs admin
/// of that tenant, or superadmin. Answers 400 for `superadmin` or a role the
/// tenant does not hold, 404 for an unknown name, and 409 when the change
/// would demote the last live admin.
#[utoipa::path(
    put,
    path = "/tenants/{tenant}/users/{name}/role",
    params(
        ("tenant" = String, Path, description = "the tenant's name"),
        ("name" = String, Path, description = "the user's email address")
    ),
    security(("bearer" = [])),
    request_body = RoleChange,
    responses(
        (status = OK, description = "the user with its new role", body = UserView),
        (status = BAD_REQUEST, description = "the tenant does not hold the role", body = String),
        (status = UNAUTHORIZED, description = "no valid access token", body = String),
        (status = FORBIDDEN, description = "the caller is below admin, or of another tenant", body = String),
        (status = NOT_FOUND, description = "no such tenant or user", body = String),
        (status = CONFLICT, description = "the user is the tenant's last live admin", body = String)
    )
)]
pub async fn change_role(
    caller: AtLeast<role::Admin>,
    State(state): State<AppState>,
    Path((tenant, name)): Path<(String, String)>,
    Json(change): Json<RoleChange>,
) -> Response {
    // The order of the checks decides which refusal a caller sees.
    //
    // 1. We refuse `superadmin`, because only the system tenant `$tk` holds
    //    that role (ADR-0028). This check needs no database, so it comes
    //    before the tenant lookup.
    // 2. We find the user before we check the role, so an unknown name is
    //    404 whatever role the body names.
    // 3. We refuse to demote the tenant's last live admin.
    // 4. We write the role, commit, and return the user with its new role.

    // 1. refuse superadmin
    if change.role == superadmin::ROLE {
        return Refused::BadRequest("no route gives the role superadmin").into_response();
    }

    let changed = async {
        // 2. find the user, check the role
        let mut tx = state.store.begin().await?;
        let tenant = scope::tenant(&mut tx, &caller.user, &caller.identity, &tenant).await?;
        // A name that fails the user-name rule matches no row, so it is 404.
        let Some(mut target) = users::find(&mut tx, &tenant.id, &name.to_lowercase()).await? else {
            return Err(Refused::NoSuchUser);
        };
        if !tenants::role_exists(&mut tx, &tenant.id, &change.role).await? {
            return Err(Refused::BadRequest("the tenant does not hold this role"));
        }

        // 3. refuse the last admin's demotion
        let demotion = target.role_name == ADMIN_ROLE && change.role != ADMIN_ROLE;
        // The count is a statement of its own, so two concurrent demotions, or
        // a demotion and a delete, on Postgres or MariaDB can both pass it.
        if demotion && users::count_admins(&mut tx, &tenant.id).await? <= 1 {
            return Err(Refused::LastAdmin);
        }

        // 4. set the role and commit
        // No token is revoked, because every request reads the role from the
        // store (ADR-0025).
        users::set_role(&mut tx, &tenant.id, &target.id, &change.role).await?;
        tx.commit().await?;
        target.role_name = change.role;
        Ok(target)
    }
    .await;
    match changed {
        Ok(target) => Json(view(target)).into_response(),
        Err(refused) => refused.into_response(),
    }
}

/// Gives the named user of the tenant a new generated password that must be
/// changed, ends the user's sessions, and returns the password. Needs admin
/// of that tenant, or superadmin. 404 for no such live user.
#[utoipa::path(
    post,
    path = "/tenants/{tenant}/users/{name}/password",
    params(
        ("tenant" = String, Path, description = "the tenant's name"),
        ("name" = String, Path, description = "the user's email address")
    ),
    security(("bearer" = [])),
    responses(
        (status = OK, description = "the user and its one-time password", body = CreatedUser),
        (status = UNAUTHORIZED, description = "no valid access token", body = String),
        (status = FORBIDDEN, description = "the caller is below admin, or of another tenant", body = String),
        (status = NOT_FOUND, description = "no such tenant or user", body = String)
    )
)]
pub async fn reset_password(
    caller: AtLeast<role::Admin>,
    State(state): State<AppState>,
    Path((tenant, name)): Path<(String, String)>,
) -> Response {
    let reset = async {
        let mut tx = state.store.begin().await?;
        let tenant = scope::tenant(&mut tx, &caller.user, &caller.identity, &tenant).await?;
        // A name that fails the user-name rule matches no row, so it is 404.
        let Some(target) = users::find(&mut tx, &tenant.id, &name.to_lowercase()).await? else {
            return Err(Refused::NoSuchUser);
        };
        // accounts::reset sets tokens_valid_after too, so the person who held
        // the old password is logged out everywhere (ADR-0025).
        let password = accounts::reset(&mut tx, &tenant.id, &target.id).await?;
        tx.commit().await?;
        Ok((target, password))
    }
    .await;
    match reset {
        Ok((target, password)) => Json(CreatedUser {
            tenant_id: target.tenant_id,
            name: target.name,
            role: target.role_name,
            password,
        })
        .into_response(),
        Err(refused) => refused.into_response(),
    }
}
