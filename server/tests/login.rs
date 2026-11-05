//! Logs in on the router served in this process, on each backend the
//! environment names, and uses what login hands out: a scenario of the
//! happy path and one refusal each, a property over defective tokens, and a
//! property over sequences of logins, refreshes, logouts and password
//! changes against a model of which tokens are live.

mod common;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use common::{
    ADMIN, TEST_KEY, TEST_PREVIOUS_KEY, backends, block_on, on_each_backend, on_pool, placeholders,
    request, serve_in_process,
};
use proptest::prelude::*;
use serde_json::{Value, json};
use sqlx::AssertSqlSafe;
use trapperkeeper_server::{Keyring, Kind};

/// The id of the default tenant, as the first migration seeds it.
const DEFAULT_TENANT: &str = "00000000-0000-7000-8000-000000000000";

/// A login with the admin's name, in any case, and its password returns a
/// bearer access token that lives an hour and a refresh token that lives a
/// week, and says the password must change. Both tokens come in the body and
/// in cookies page scripts and other sites cannot use, and the refresh cookie
/// goes to the refresh route alone. The access token reads the admin back on
/// the bearer header and on the cookie alone. A wrong password and an unknown
/// name get the same refusal.
#[test]
fn login_then_me() {
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        let credentials =
            json!({ "name": ADMIN.to_uppercase(), "password": server.admin_password });
        let response = request(
            server.addr,
            "POST",
            "/api/v1/login",
            &[],
            Some(&credentials.to_string()),
        );
        assert_eq!(response.status, 200, "{}", response.body);
        let login: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(login["token_type"], "Bearer");
        assert_eq!(login["expires_in"], 3600);
        assert_eq!(login["password_must_change"], true);
        let token = login["access_token"].as_str().expect("an access token");
        // Three base64url parts, as a JWT is.
        assert_eq!(token.split('.').count(), 3);
        assert_eq!(login["refresh_expires_in"], 7 * 24 * 3600);
        let refresh = login["refresh_token"].as_str().expect("a refresh token");
        assert_eq!(refresh.split('.').count(), 3);
        let set_cookie = response.header_values("set-cookie");
        assert_eq!(set_cookie.len(), 2, "{set_cookie:?}");
        for (value, path, max_age) in [
            (format!("tk_access={token}"), "Path=/", "Max-Age=3600"),
            (
                format!("tk_refresh={refresh}"),
                "Path=/api/v1/refresh",
                "Max-Age=604800",
            ),
        ] {
            // The two cookies come in no fixed order.
            let header = set_cookie
                .iter()
                .find(|header| header.starts_with(&value))
                .unwrap_or_else(|| panic!("{value} not in {set_cookie:?}"));
            let attributes: Vec<&str> = header.split("; ").collect();
            for attribute in ["HttpOnly", "SameSite=Strict", "Secure", path, max_age] {
                assert!(
                    attributes.contains(&attribute),
                    "{attribute} not in {header:?}"
                );
            }
        }

        let bearer = format!("Bearer {token}");
        let response = request(
            server.addr,
            "GET",
            "/api/v1/me",
            &[("Authorization", &bearer)],
            None,
        );
        assert_eq!(response.status, 200, "{}", response.body);
        let me: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(
            me,
            json!({ "name": ADMIN, "role": "admin", "password_must_change": true })
        );

        let cookie = format!("tk_access={token}");
        let response = request(
            server.addr,
            "GET",
            "/api/v1/me",
            &[("Cookie", &cookie)],
            None,
        );
        assert_eq!(response.status, 200, "{}", response.body);

        let response = request(server.addr, "GET", "/api/v1/me", &[], None);
        assert_eq!(response.status, 401);
        assert_eq!(response.header_values("www-authenticate"), ["Bearer"]);

        for wrong in [
            json!({ "name": ADMIN, "password": "not the password" }),
            json!({ "name": "nobody", "password": server.admin_password }),
        ] {
            let response = request(
                server.addr,
                "POST",
                "/api/v1/login",
                &[],
                Some(&wrong.to_string()),
            );
            assert_eq!(response.status, 401);
            assert_eq!(response.body, "unauthorized");
        }
    });
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
            .bind(DEFAULT_TENANT)
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
            let mut token = signer.issue(Kind::Access, &user_id, DEFAULT_TENANT, issued);
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
    /// Mint a token of a kind that expired long ago, as if a week had passed.
    Expired { refresh: bool },
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => any::<bool>().prop_map(|right_password| Op::Login { right_password }),
        3 => any::<usize>().prop_map(|token| Op::Me { token }),
        1 => any::<bool>().prop_map(|refresh| Op::Expired { refresh }),
    ]
}

/// The model: the admin's password, the current revocation epoch, and every
/// token held.
struct Model {
    password: String,
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

/// Logs in with `password` and returns the response.
fn log_in(server: &common::InProcess, password: &str) -> common::Response {
    let credentials = json!({ "name": ADMIN, "password": password });
    request(
        server.addr,
        "POST",
        "/api/v1/login",
        &[],
        Some(&credentials.to_string()),
    )
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 12,
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// For any sequence of logins and reads of me, each token is accepted
    /// exactly when the model says it is live: of the kind the route asks
    /// for, issued in the current revocation epoch, and not expired.
    #[test]
    fn tokens_follow_the_model(ops in proptest::collection::vec(op(), 0..=8)) {
        for backend in backends() {
            let server = serve_in_process(backend);
            let user_id = admin_id(server.database());
            let own = block_on(Keyring::from_store(&server.store, TEST_KEY, None))
                .expect("the keys derive");
            let mut model = Model {
                password: server.admin_password.clone(),
                epoch: 0,
                held: Vec::new(),
            };
            for op in &ops {
                match op {
                    Op::Login { right_password } => {
                        let password = if *right_password { model.password.clone() } else { "wrong".to_owned() };
                        let response = log_in(&server, &password);
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
                    }
                    Op::Expired { refresh } => {
                        let kind = if *refresh { Kind::Refresh } else { Kind::Access };
                        let issued = SystemTime::now() - Duration::from_hours(8 * 24);
                        model.held.push(Held {
                            value: own.issue(kind, &user_id, DEFAULT_TENANT, issued),
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
