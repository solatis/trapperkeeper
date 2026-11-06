//! Logs in on the router served in this process, on each backend the
//! environment names, and uses what login hands out: a scenario of the
//! happy path and one refusal each, a property over defective tokens, and a
//! property over sequences of logins, refreshes, logouts and password
//! changes against a model of which tokens are live.

mod common;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use common::{
    ADMIN, SYSTEM_TENANT, SYSTEM_TENANT_ID, TEST_KEY, TEST_PREVIOUS_KEY, backends, block_on,
    on_each_backend, on_pool, placeholders, request, serve_in_process,
};
use proptest::prelude::*;
use serde_json::{Value, json};
use sqlx::AssertSqlSafe;
use trapperkeeper_server::{Keyring, Kind};

/// A login with the superadmin's tenant and name, in any case, and its
/// password returns a bearer access token that lives an hour and a refresh
/// token that lives a week, and says the password must change. Both tokens
/// come in the body and in cookies page scripts and other sites cannot use,
/// and the refresh cookie goes to the refresh route alone. The access token
/// reads the superadmin back on the bearer header and on the cookie alone. An
/// unknown tenant, the superadmin's name and password under another tenant, a
/// wrong password and an unknown name get the same refusal.
#[test]
fn login_then_me() {
    // 1. log in with the tenant and the name in upper case
    // 2. check the body and the two cookies
    // 3. read me with the bearer header, then with the cookie alone
    // 4. refresh with the refresh cookie alone
    // 5. read me with no token
    // 6. try four logins that must fail
    on_each_backend(|backend| {
        let server = serve_in_process(backend);

        // 1. Upper case, so the test sees that login lowercases the tenant
        //    and the name before it looks them up (doc/adr/0020).
        let response = log_in(
            &server,
            &SYSTEM_TENANT.to_uppercase(),
            &ADMIN.to_uppercase(),
            &server.admin_password,
        );
        assert_eq!(response.status, 200, "{}", response.body);

        // 2. The lifetimes are the access token's hour and the refresh
        //    token's week, and the superadmin's generated password must
        //    change.
        let login: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(login["token_type"], "Bearer");
        assert_eq!(login["expires_in"], 3600);
        assert_eq!(login["refresh_expires_in"], 7 * 24 * 3600);
        assert_eq!(login["password_must_change"], true);
        let token = login["access_token"].as_str().expect("an access token");
        let refresh = login["refresh_token"].as_str().expect("a refresh token");
        // Three base64url parts, as a JWT is.
        assert_eq!(token.split('.').count(), 3);
        assert_eq!(refresh.split('.').count(), 3);
        let set_cookie = response.header_values("set-cookie");
        assert_eq!(set_cookie.len(), 2, "{set_cookie:?}");
        assert_token_cookie(
            &set_cookie,
            &format!("tk_access={token}"),
            "Path=/",
            "Max-Age=3600",
        );
        assert_token_cookie(
            &set_cookie,
            &format!("tk_refresh={refresh}"),
            "Path=/api/v1/refresh",
            "Max-Age=604800",
        );

        // 3. The CLI sends the header and the browser sends the cookie, so
        //    each one alone must authenticate.
        let bearer = format!("Bearer {token}");
        let response = me(&server, &[("Authorization", &bearer)]);
        assert_eq!(response.status, 200, "{}", response.body);
        let body: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(
            body,
            json!({ "name": ADMIN, "role": "superadmin", "password_must_change": true })
        );
        let cookie = format!("tk_access={token}");
        assert_eq!(me(&server, &[("Cookie", &cookie)]).status, 200);

        // 4. The refresh cookie alone yields a new access token that works.
        let cookie = format!("tk_refresh={refresh}");
        let response = request(
            server.addr,
            "POST",
            "/api/v1/refresh",
            &[("Cookie", &cookie)],
            None,
        );
        assert_eq!(response.status, 200, "{}", response.body);
        let refreshed: Value = serde_json::from_str(&response.body).expect("a JSON body");
        let fresh = refreshed["access_token"].as_str().expect("an access token");
        let bearer = format!("Bearer {fresh}");
        assert_eq!(me(&server, &[("Authorization", &bearer)]).status, 200);

        // 5. A request with no token gets the 401 that asks for a bearer
        //    token (RFC 6750, 3).
        let response = me(&server, &[]);
        assert_eq!(response.status, 401);
        assert_eq!(response.header_values("www-authenticate"), ["Bearer"]);

        // 6. A second tenant, inserted through sqlx because no route creates
        //    tenants, so the superadmin's credentials are refused under its
        //    name.
        on_pool!(server.database(), |pool| {
            sqlx::query(
                "INSERT INTO tenants (tenant_id, tenant_name, tenant_title) \
                 VALUES ('00000000-0000-7000-8000-0000000000ff', 'other', 'Other')",
            )
            .execute(&pool)
            .await
            .expect("the tenant is inserted");
        });
        // An unknown tenant, another tenant, a wrong password and an unknown
        // name. All four get the same body, so a response does not say which
        // part was wrong.
        let password = server.admin_password.as_str();
        for (tenant, name, password) in [
            ("nowhere", ADMIN, password),
            ("other", ADMIN, password),
            (SYSTEM_TENANT, ADMIN, "not the password"),
            (SYSTEM_TENANT, "nobody", password),
        ] {
            let response = log_in(&server, tenant, name, password);
            assert_eq!(response.status, 401);
            assert_eq!(response.body, "unauthorized");
        }
    });
}

/// Logs in to `tenant` as `name` with `password` and returns the response.
fn log_in(
    server: &common::InProcess,
    tenant: &str,
    name: &str,
    password: &str,
) -> common::Response {
    let credentials = json!({ "tenant": tenant, "name": name, "password": password });
    request(
        server.addr,
        "POST",
        "/api/v1/login",
        &[],
        Some(&credentials.to_string()),
    )
}

/// Reads me with `headers` and returns the response.
fn me(server: &common::InProcess, headers: &[(&str, &str)]) -> common::Response {
    request(server.addr, "GET", "/api/v1/me", headers, None)
}

/// The `Set-Cookie` header among `set_cookie` that starts with `value` has
/// the attributes every token cookie has, plus `path` and `max_age`.
fn assert_token_cookie(set_cookie: &[&str], value: &str, path: &str, max_age: &str) {
    // The cookies come in no fixed order.
    let header = set_cookie
        .iter()
        .find(|header| header.starts_with(value))
        .unwrap_or_else(|| panic!("{value} not in {set_cookie:?}"));
    let attributes: Vec<&str> = header.split("; ").collect();
    for attribute in ["HttpOnly", "SameSite=Strict", "Secure", path, max_age] {
        assert!(
            attributes.contains(&attribute),
            "{attribute} not in {header:?}"
        );
    }
}

/// The defects a generated token can have. Each one alone makes the server
/// refuse it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Defect {
    /// One character of the signature changed.
    FlippedSignature,
    /// Signed by a key the server's ring does not hold.
    UnknownKey,
    /// Issued two hours ago, past the one-hour lifetime and the leeway.
    Expired,
    /// The user's cutoff was moved past the token's issue time.
    BeforeCutoff,
    /// The user was deleted after the token was issued.
    DeletedUser,
}

const DEFECTS: [Defect; 5] = [
    Defect::FlippedSignature,
    Defect::UnknownKey,
    Defect::Expired,
    Defect::BeforeCutoff,
    Defect::DeletedUser,
];

/// The signature part with its last character changed to another base64url
/// character.
fn flip_signature(token: &str) -> String {
    let (head, signature) = token.rsplit_once('.').expect("a signed token");
    let last = signature.chars().last().expect("a signature");
    let other = if last == 'A' { 'B' } else { 'A' };
    let mut flipped: String = signature.chars().take(signature.len() - 1).collect();
    flipped.push(other);
    format!("{head}.{flipped}")
}

/// The admin's user id, read beside the server.
fn admin_id(database: &common::TestDatabase) -> String {
    let select = placeholders(
        database.backend,
        "SELECT user_id FROM users WHERE tenant_id = $1 AND user_name = $2",
    );
    on_pool!(database, |pool| {
        sqlx::query_scalar(AssertSqlSafe(select))
            .bind(SYSTEM_TENANT_ID)
            .bind(ADMIN)
            .fetch_one(&pool)
            .await
            .expect("the admin is read")
    })
}

/// Sets `column` of the admin to `value`, as a route will once one writes it.
fn set_admin(database: &common::TestDatabase, column: &'static str, value: i64) {
    let update = placeholders(
        database.backend,
        match column {
            "tokens_valid_after" => "UPDATE users SET tokens_valid_after = $1 WHERE user_id = $2",
            "deleted_at" => "UPDATE users SET deleted_at = $1 WHERE user_id = $2",
            _ => unreachable!("only two columns are written"),
        },
    );
    let id = admin_id(database);
    on_pool!(database, |pool| {
        sqlx::query(AssertSqlSafe(update))
            .bind(value)
            .bind(&id)
            .execute(&pool)
            .await
            .expect("the admin is updated");
    });
}

proptest! {
    // 12 cases rather than proptest's 256: each case builds a server and
    // derives three keyrings, with a 64 MiB hash each, on every backend. No
    // regression file, so a failure leaves nothing untracked in the tree; the
    // failing seed is in the output.
    #![proptest_config(ProptestConfig {
        cases: 12,
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// A token passes only when its signature, key, expiry, user and cutoff
    /// are all valid: the server answers `me` with 200 when the token has no
    /// defect and 401 otherwise. A ring that holds the old key as previous
    /// still checks the old key's tokens, and a ring without it does not.
    #[test]
    fn a_token_passes_only_when_every_check_holds(
        defects in proptest::sample::subsequence(DEFECTS.to_vec(), 0..=DEFECTS.len())
    ) {
        let has = |defect: Defect| defects.contains(&defect);
        for backend in backends() {
            let server = serve_in_process(backend);
            let user_id = admin_id(server.database());
            // The server's own ring, the ring of another passphrase, and a
            // ring that signs with the other and still checks the server's.
            let (own, other, rotated) = block_on(async {
                let store = &server.store;
                (
                    Keyring::from_store(store, TEST_KEY, None).await.expect("the keys derive"),
                    Keyring::from_store(store, TEST_PREVIOUS_KEY, None).await.expect("the keys derive"),
                    Keyring::from_store(store, TEST_PREVIOUS_KEY, Some(TEST_KEY)).await.expect("the keys derive"),
                )
            });

            let now = SystemTime::now();
            let issued = if has(Defect::Expired) { now - Duration::from_hours(2) } else { now };
            let signer = if has(Defect::UnknownKey) { &other } else { &own };
            let mut token = signer.issue(Kind::Access, &user_id, SYSTEM_TENANT_ID, issued);
            if has(Defect::FlippedSignature) {
                token = flip_signature(&token);
            }
            let now_micros = i64::try_from(
                now.duration_since(UNIX_EPOCH).expect("the clock is past 1970").as_micros(),
            )
            .expect("a small number");
            if has(Defect::BeforeCutoff) {
                set_admin(server.database(), "tokens_valid_after", now_micros + 1);
            }
            if has(Defect::DeletedUser) {
                set_admin(server.database(), "deleted_at", now_micros);
            }

            let bearer = format!("Bearer {token}");
            let response = request(server.addr, "GET", "/api/v1/me", &[("Authorization", &bearer)], None);
            let expected = if defects.is_empty() { 200 } else { 401 };
            prop_assert_eq!(response.status, expected, "{:?} on {}: {}", defects, backend, response.body);

            // The rings see the token itself, not the user, so only the
            // signature, the key and the expiry matter to them.
            let intact = !has(Defect::FlippedSignature) && !has(Defect::Expired);
            prop_assert_eq!(rotated.check(Kind::Access, &token).is_ok(), intact, "{:?} on {}", defects, backend);
            prop_assert_eq!(other.check(Kind::Access, &token).is_ok(), intact && has(Defect::UnknownKey), "{:?} on {}", defects, backend);
        }
    }
}

/// What the model knows about a token the server handed out or the test
/// minted.
#[derive(Debug, Clone)]
struct Held {
    value: String,
    kind: Kind,
    /// The revocation epoch the token was issued in; a logout or a password
    /// change starts the next one.
    epoch: u32,
    expired: bool,
}

/// One step of a generated sequence. A token index picks among the tokens
/// held so far, and picks a garbage token when none is held.
#[derive(Debug, Clone)]
enum Op {
    /// Log in with the right password, or a wrong one.
    Login { right_password: bool },
    /// Read me with a held token.
    Me { token: usize },
    /// Trade a held token for a new access token.
    Refresh { token: usize },
    /// Log out with a held token.
    Logout { token: usize },
    /// Change the password with a held token, the right or a wrong current
    /// password, and a new one of the given length.
    ChangePassword {
        token: usize,
        right_current: bool,
        new_len: usize,
    },
    /// Mint a token of a kind that expired long ago, as if a week had passed.
    Expired { refresh: bool },
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => any::<bool>().prop_map(|right_password| Op::Login { right_password }),
        3 => any::<usize>().prop_map(|token| Op::Me { token }),
        2 => any::<usize>().prop_map(|token| Op::Refresh { token }),
        1 => any::<usize>().prop_map(|token| Op::Logout { token }),
        1 => (any::<usize>(), any::<bool>(), 8_usize..=20).prop_map(|(token, right_current, new_len)| {
            Op::ChangePassword { token, right_current, new_len }
        }),
        1 => any::<bool>().prop_map(|refresh| Op::Expired { refresh }),
    ]
}

/// The model: the admin's password, whether it must still be changed, the
/// current revocation epoch, and every token held.
struct Model {
    password: String,
    must_change: bool,
    epoch: u32,
    held: Vec<Held>,
}

impl Model {
    /// The held token at `index`, or a garbage token when none is held.
    fn token(&self, index: usize) -> Held {
        self.held
            .get(index % self.held.len().max(1))
            .cloned()
            .unwrap_or(Held {
                value: "not.a.token".to_owned(),
                kind: Kind::Access,
                epoch: 0,
                expired: true,
            })
    }

    /// Whether the server accepts `token` as a token of `kind` now.
    fn live(&self, token: &Held, kind: Kind) -> bool {
        token.kind == kind && token.epoch == self.epoch && !token.expired
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 12,
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// For any sequence of logins, refreshes, logouts, password changes and
    /// reads of me, each token is accepted exactly when the model says it is
    /// live: of the kind the route asks for, issued in the current revocation
    /// epoch, and not expired. A refresh hands out an access token of the
    /// current epoch. A logout starts the next epoch, so every token held
    /// before it dies, and clears both cookies. A password change needs the
    /// current password and a new one of 12 to 1024 characters; it starts the
    /// next epoch, clears the must-change flag, and hands out tokens of the
    /// new epoch.
    #[test]
    fn tokens_follow_the_model(ops in proptest::collection::vec(op(), 0..=8)) {
        for backend in backends() {
            let server = serve_in_process(backend);
            let user_id = admin_id(server.database());
            let own = block_on(Keyring::from_store(&server.store, TEST_KEY, None))
                .expect("the keys derive");
            let mut model = Model {
                password: server.admin_password.clone(),
                must_change: true,
                epoch: 0,
                held: Vec::new(),
            };
            for op in &ops {
                match op {
                    Op::Login { right_password } => {
                        let password = if *right_password { model.password.clone() } else { "wrong".to_owned() };
                        let response = log_in(&server, SYSTEM_TENANT, ADMIN, &password);
                        if !*right_password {
                            prop_assert_eq!(response.status, 401, "{:?} on {}", op, backend);
                            continue;
                        }
                        prop_assert_eq!(response.status, 200, "{:?} on {}: {}", op, backend, response.body);
                        let login: Value = serde_json::from_str(&response.body).expect("a JSON body");
                        for (field, kind) in [("access_token", Kind::Access), ("refresh_token", Kind::Refresh)] {
                            model.held.push(Held {
                                value: login[field].as_str().expect("a token").to_owned(),
                                kind,
                                epoch: model.epoch,
                                expired: false,
                            });
                        }
                    }
                    Op::Me { token } => {
                        let held = model.token(*token);
                        let bearer = format!("Bearer {}", held.value);
                        let response = request(server.addr, "GET", "/api/v1/me", &[("Authorization", &bearer)], None);
                        let expected = if model.live(&held, Kind::Access) { 200 } else { 401 };
                        prop_assert_eq!(response.status, expected, "{:?} {:?} on {}: {}", op, held, backend, response.body);
                        if response.status == 200 {
                            let body: Value = serde_json::from_str(&response.body).expect("a JSON body");
                            prop_assert_eq!(body["password_must_change"].as_bool(), Some(model.must_change));
                        }
                    }
                    Op::ChangePassword { token, right_current, new_len } => {
                        let held = model.token(*token);
                        let current = if *right_current { model.password.clone() } else { "wrong".to_owned() };
                        let new_password = "n".repeat(*new_len);
                        let bearer = format!("Bearer {}", held.value);
                        let body = json!({ "current_password": current, "new_password": new_password }).to_string();
                        let response = request(server.addr, "POST", "/api/v1/password", &[("Authorization", &bearer)], Some(&body));
                        let expected = if !model.live(&held, Kind::Access) || !*right_current {
                            401
                        } else if *new_len < 12 {
                            400
                        } else {
                            200
                        };
                        prop_assert_eq!(response.status, expected, "{:?} {:?} on {}: {}", op, held, backend, response.body);
                        if response.status == 200 {
                            model.epoch += 1;
                            model.password = new_password;
                            model.must_change = false;
                            let login: Value = serde_json::from_str(&response.body).expect("a JSON body");
                            prop_assert_eq!(login["password_must_change"].as_bool(), Some(false));
                            for (field, kind) in [("access_token", Kind::Access), ("refresh_token", Kind::Refresh)] {
                                model.held.push(Held {
                                    value: login[field].as_str().expect("a token").to_owned(),
                                    kind,
                                    epoch: model.epoch,
                                    expired: false,
                                });
                            }
                        }
                    }
                    Op::Refresh { token } => {
                        let held = model.token(*token);
                        let body = json!({ "refresh_token": held.value }).to_string();
                        let response = request(server.addr, "POST", "/api/v1/refresh", &[], Some(&body));
                        let expected = if model.live(&held, Kind::Refresh) { 200 } else { 401 };
                        prop_assert_eq!(response.status, expected, "{:?} {:?} on {}: {}", op, held, backend, response.body);
                        if response.status == 200 {
                            let refreshed: Value = serde_json::from_str(&response.body).expect("a JSON body");
                            model.held.push(Held {
                                value: refreshed["access_token"].as_str().expect("a token").to_owned(),
                                kind: Kind::Access,
                                epoch: model.epoch,
                                expired: false,
                            });
                        }
                    }
                    Op::Logout { token } => {
                        let held = model.token(*token);
                        let bearer = format!("Bearer {}", held.value);
                        let response = request(server.addr, "POST", "/api/v1/logout", &[("Authorization", &bearer)], None);
                        let expected = if model.live(&held, Kind::Access) { 200 } else { 401 };
                        prop_assert_eq!(response.status, expected, "{:?} {:?} on {}: {}", op, held, backend, response.body);
                        if response.status == 200 {
                            let set_cookie = response.header_values("set-cookie");
                            prop_assert_eq!(set_cookie.len(), 2, "{:?}", set_cookie);
                            for header in &set_cookie {
                                prop_assert!(header.contains("Max-Age=0"), "{header:?}");
                            }
                            model.epoch += 1;
                        }
                    }
                    Op::Expired { refresh } => {
                        let kind = if *refresh { Kind::Refresh } else { Kind::Access };
                        let issued = SystemTime::now() - Duration::from_hours(8 * 24);
                        model.held.push(Held {
                            value: own.issue(kind, &user_id, SYSTEM_TENANT_ID, issued),
                            kind,
                            epoch: model.epoch,
                            expired: true,
                        });
                    }
                }
            }
        }
    }
}
