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

/// Why a user route refused a request or failed. Each variant converts into
/// the response the caller gets. The list route, which writes nothing, also
/// returns it.
enum Refused {
    /// The tenant in the path failed the tenant rule, and `scope::Refused`
    /// gives the response: 404, 403 or 500.
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
    /// True while the user still has to replace a generated password at the
    /// next login.
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
    // `names::user_name` checks the shape and returns the name lowercased,
    // which is the form the store keeps and the response returns.
    let name = match names::user_name(&request.name) {
        Ok(name) => name,
        Err(refused) => return Refused::BadRequest(refused.0).into_response(),
    };
    // `superadmin` is refused by name before any read, because `$tk` holds
    // the role alone and a second superadmin was rejected (ADR-0028).
    if request.role == superadmin::ROLE {
        return Refused::BadRequest("no route gives the role superadmin").into_response();
    }

    let created = async {
        let mut tx = state.store.begin().await?;
        let tenant = scope::tenant(&mut tx, &caller.user, &caller.identity, &tenant).await?;
        // A role the tenant does not hold would fail the insert on the NOT
        // NULL role id, which surfaces as a 500, so the route refuses it with
        // 400 first.
        if !tenants::role_exists(&mut tx, &tenant.id, &request.role).await? {
            return Err(Refused::BadRequest("the tenant does not hold this role"));
        }
        // The handler creates the user through `accounts::create`, so the
        // user gets a generated password that must be changed at the first
        // login.
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
        // The generated password reaches the person who created the user
        // only in this response (ADR-0028).
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
/// superadmin, or the user itself. Answers 404 for a name that no live user
/// has only to a caller at admin or above, and any other caller gets 403
/// whether or not the name exists. Answers 409 when the user is the tenant's
/// last live admin, also when the user deletes itself.
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
    // The handler deletes the named user of the tenant when the caller may.
    // A caller at admin or above, the superadmin included, deletes any live
    // user, and any other user of the tenant deletes only itself. It takes
    // `Caller` instead of `AtLeast<Admin>`, because a user below admin may
    // delete itself. It decides the permission after the lookup, refuses the
    // delete of the last live admin, and writes and commits last, so every
    // refusal writes nothing.
    //
    // 1. We open the transaction, resolve the tenant and find the live
    //    target by its lowercased name. `scope::tenant` refuses a member of
    //    another tenant with 403, so `Caller` admits no one outside the
    //    tenant.
    // 2. We decide from the caller's rank and the target. A caller at admin
    //    or above gets the target or 404, and any other caller gets its own
    //    user or 403, so a caller below admin cannot tell whether another
    //    name exists. This comes after the lookup, because only the found
    //    user's id shows whether the caller is the target.
    // 3. We refuse with 409 when the target holds the role admin and the
    //    tenant has at most one live admin, also when the target is the
    //    caller, because the user routes refuse a change that would leave
    //    the tenant without a live admin.
    // 4. We mark the user deleted at now and commit. The user's tokens need
    //    no revocation, because every request reads the user and refuses a
    //    deleted one (ADR-0025).

    let deleted = async {
        // 1. resolve the tenant and the target
        let mut tx = state.store.begin().await?;
        let tenant = scope::tenant(&mut tx, &caller.user, &caller.identity, &tenant).await?;
        // The name is lowercased as the store keeps it and not checked for
        // shape, because a name that fails the shape matches no row (ADR-0020).
        let found = users::find(&mut tx, &tenant.id, &name.to_lowercase()).await?;

        // 2. decide the permission
        // A role that is not on the ladder ranks below admin, so its holder
        // may delete only itself.
        let deletes_anyone =
            Rank::of(&caller.user.role_name).is_some_and(|rank| rank >= Rank::Admin);
        let target = match found {
            Some(target) if deletes_anyone || target.id == caller.user.id => target,
            None if deletes_anyone => return Err(Refused::NoSuchUser),
            _ => return Err(Refused::Forbidden),
        };

        // 3. refuse the last admin
        // The count runs in the write's transaction as a statement of its own,
        // so two concurrent deletes on Postgres or MariaDB can both count two
        // and both pass.
        if target.role_name == ADMIN_ROLE && users::count_admins(&mut tx, &tenant.id).await? <= 1 {
            return Err(Refused::LastAdmin);
        }

        // 4. delete and commit
        // Zero rows changed means that another request deleted the user
        // between the find and this write, which the route answers as 404.
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
/// tenant does not hold, 404 for a name that no live user has, and 409 when
/// the change would demote the last live admin.
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
    // The handler gives the named user one of the tenant's roles. It runs its
    // checks in the order that decides which refusal a caller sees, writes
    // the role and commits last, and returns the user with the new role
    // without reading it again.
    //
    // 1. We refuse the role superadmin by name with 400 before any read,
    //    because `$tk` holds the role alone and a second superadmin was
    //    rejected (ADR-0028).
    // 2. We open the transaction, resolve the tenant, find the live user by
    //    its lowercased name, and then check that the tenant holds the role.
    //    The user is found before the role is checked, so an unknown name is
    //    404 whatever role the body names.
    // 3. We refuse with 409 when the change demotes an admin and the tenant
    //    has at most one live admin. Only a demotion from admin can leave the
    //    tenant without a live admin, so a promotion or a change between two
    //    other roles skips the count.
    // 4. We write the role, commit, and set the role on the user that step 2
    //    read, so the response shows the new role.

    // 1. refuse superadmin
    if change.role == superadmin::ROLE {
        return Refused::BadRequest("no route gives the role superadmin").into_response();
    }

    let changed = async {
        // 2. find the user, check the role
        let mut tx = state.store.begin().await?;
        let tenant = scope::tenant(&mut tx, &caller.user, &caller.identity, &tenant).await?;
        let Some(mut target) = users::find(&mut tx, &tenant.id, &name.to_lowercase()).await? else {
            return Err(Refused::NoSuchUser);
        };
        if !tenants::role_exists(&mut tx, &tenant.id, &change.role).await? {
            return Err(Refused::BadRequest("the tenant does not hold this role"));
        }

        // 3. refuse the last admin's demotion
        // The count runs in the write's transaction as a statement of its own,
        // so two concurrent demotions, or a demotion and a delete, on Postgres
        // or MariaDB can both count two and both pass.
        let demotion = target.role_name == ADMIN_ROLE && change.role != ADMIN_ROLE;
        if demotion && users::count_admins(&mut tx, &tenant.id).await? <= 1 {
            return Err(Refused::LastAdmin);
        }

        // 4. set the role and commit
        // The new role takes effect at the user's next request, because every
        // request reads the user's role from the store (ADR-0025). That is why
        // no token is revoked.
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
