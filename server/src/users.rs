//! The admin's routes for the users of a tenant, under
//! `/tenants/{tenant}/users`. A tenant keeps at least one live admin: the
//! routes refuse the delete or demotion that would leave none.

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

/// Why a user route changed nothing. It converts into the response the
/// caller gets.
enum Refused {
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

/// A user as the list shows it.
#[derive(Serialize, ToSchema)]
pub struct UserView {
    tenant_id: String,
    name: String,
    role: String,
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
        let scope = scope::resolve(&mut tx, &caller.user, &tenant).await?;
        Ok::<_, Refused>(users::list(&mut tx, &scope.tenant.id).await?)
    }
    .await;
    match listed {
        Ok(listed) => Json(listed.into_iter().map(view).collect::<Vec<_>>()).into_response(),
        Err(refused) => refused.into_response(),
    }
}

/// `user` as the list shows it.
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

/// What a create or a reset hands back: the generated password, shown once.
#[derive(Serialize, ToSchema)]
pub struct CreatedUser {
    tenant_id: String,
    name: String,
    role: String,
    password: String,
}

/// Creates a user of the tenant with a generated password that must be
/// changed. Needs admin of that tenant, or superadmin. 400 for a bad name, a
/// role the tenant does not hold, or `superadmin`; 409 when a live user has
/// the name.
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
    // 1. check the name's shape and refuse the superadmin role
    // 2. open the transaction, resolve the scope, check the tenant holds the role
    // 3. create the user through accounts::create
    // 4. commit and return the password

    // 1. `superadmin` is refused by name before any read, because `$tk` holds
    //    it alone and a second superadmin was rejected (doc/adr/0028).
    let name = match names::user_name(&request.name) {
        Ok(name) => name,
        Err(refused) => return Refused::BadRequest(refused.0).into_response(),
    };
    if request.role == superadmin::ROLE {
        return Refused::BadRequest("no route gives the role superadmin").into_response();
    }

    let created = async {
        let mut tx = state.store.begin().await?;
        let scope = scope::resolve(&mut tx, &caller.user, &tenant).await?;
        // 2. The role is checked before the insert, because a role the tenant
        //    does not hold would otherwise fail the insert's NOT NULL role id,
        //    which is a 500.
        if !tenants::role_exists(&mut tx, &scope.tenant.id, &request.role).await? {
            return Err(Refused::BadRequest("the tenant does not hold this role"));
        }
        // 3. Through accounts::create, so the user gets a generated password
        //    that must be changed.
        let password = accounts::create(
            &mut tx,
            NewAccount {
                tenant_id: &scope.tenant.id,
                name: &name,
                role_name: &request.role,
            },
        )
        .await?;
        tx.commit().await?;
        Ok((scope.tenant.id, password))
    }
    .await;

    // 4. The password is in this response and nowhere else (doc/adr/0028).
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

/// Deletes the named user of the tenant. The caller is an admin of that
/// tenant, the superadmin, or the user itself. 404 for no such live user, 409
/// when the user is the tenant's last live admin.
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
    // 1. resolve the scope as any caller, then find the live user by its lowercased name
    // 2. allow the call when the caller ranks admin or above, or is the target
    // 3. when the target's role is admin, count the tenant's admins
    // 4. mark it deleted
    // 5. commit

    let deleted = async {
        let mut tx = state.store.begin().await?;
        // 1. `Caller` rather than `AtLeast<Admin>`, because a user below admin
        //    may delete itself; the scope rule still applies, so a member of
        //    another tenant gets 403.
        let scope = scope::resolve(&mut tx, &caller.user, &tenant).await?;
        // The name is lowercased as the store keeps it and not checked for
        // shape, because a name that fails the shape matches no row (doc/adr/0020).
        let found = users::find(&mut tx, &scope.tenant.id, &name.to_lowercase()).await?;

        // 2. The target is found before the permission is checked, so a
        //    non-admin probing another user's name gets 403 whether or not the
        //    name exists; the 404 goes only to callers allowed to delete anyone.
        let any_user = Rank::of(&caller.user.role_name).is_some_and(|rank| rank >= Rank::Admin);
        let target = match found {
            Some(target) if any_user || target.id == caller.user.id => target,
            None if any_user => return Err(Refused::NoSuchUser),
            _ => return Err(Refused::Forbidden),
        };

        // 3. A count of one means the target is the last live admin, which is
        //    409, also when the target is the caller. The count runs in the
        //    write's transaction; two concurrent deletes on Postgres or MariaDB
        //    can both count two and both pass, which is accepted over a row
        //    lock SQLite does not have.
        if target.role_name == ADMIN_ROLE
            && users::count_admins(&mut tx, &scope.tenant.id).await? <= 1
        {
            return Err(Refused::LastAdmin);
        }

        // 4. The user's tokens need no revocation, because every request reads
        //    the user and a deleted one is refused.
        if !users::delete(&mut tx, &scope.tenant.id, &target.id, now_micros()).await? {
            return Err(Refused::NoSuchUser);
        }

        // 5.
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

/// Gives the named user of the tenant another of the tenant's roles. Needs
/// admin of that tenant, or superadmin. 400 for a role the tenant does not
/// hold or `superadmin`; 404 for no such live user; 409 when the change would
/// demote the last live admin.
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
    // 1. refuse the superadmin role by name
    // 2. resolve the scope, find the live user, check the tenant holds the role
    // 3. when the user is an admin and the new role is not, count the admins
    // 4. set the role and commit

    // 1. `superadmin` is refused by name before any read, because `$tk` holds
    //    it alone and a second superadmin was rejected (doc/adr/0028).
    if change.role == superadmin::ROLE {
        return Refused::BadRequest("no route gives the role superadmin").into_response();
    }

    let changed = async {
        let mut tx = state.store.begin().await?;
        // 2.
        let scope = scope::resolve(&mut tx, &caller.user, &tenant).await?;
        let Some(mut target) = users::find(&mut tx, &scope.tenant.id, &name.to_lowercase()).await?
        else {
            return Err(Refused::NoSuchUser);
        };
        if !tenants::role_exists(&mut tx, &scope.tenant.id, &change.role).await? {
            return Err(Refused::BadRequest("the tenant does not hold this role"));
        }

        // 3. Only a demotion needs the count: a change to admin, or between
        //    two other roles, cannot empty the admins.
        let demotion = target.role_name == ADMIN_ROLE && change.role != ADMIN_ROLE;
        if demotion && users::count_admins(&mut tx, &scope.tenant.id).await? <= 1 {
            return Err(Refused::LastAdmin);
        }

        // 4. The role takes effect at the user's next request, because the
        //    caller's role is read from the store each time, so no token is
        //    revoked (doc/adr/0025).
        users::set_role(&mut tx, &scope.tenant.id, &target.id, &change.role).await?;
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
        let scope = scope::resolve(&mut tx, &caller.user, &tenant).await?;
        let Some(target) = users::find(&mut tx, &scope.tenant.id, &name.to_lowercase()).await?
        else {
            return Err(Refused::NoSuchUser);
        };
        // accounts::reset sets tokens_valid_after too, so the person who held
        // the old password is logged out everywhere (doc/adr/0025).
        let password = accounts::reset(&mut tx, &scope.tenant.id, &target.id).await?;
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
