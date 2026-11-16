//! Logs in on the router served in this process, on each backend, and uses
//! what login hands out: a scenario that uses both tokens and checks four
//! kinds of wrong credentials, a property over defective tokens, and a
//! property over sequences of logins, refreshes, logouts and password changes
//! against a model of which tokens are live.

mod common;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use common::{
    ADMIN, SYSTEM_TENANT, SYSTEM_TENANT_ID, TEST_KEY, TEST_PREVIOUS_KEY, backends, block_on, call,
    on_each_backend, on_pool, placeholders, request, serve_in_process, superadmin_token,
};
use proptest::prelude::*;
use serde_json::{Value, json};
use sqlx::AssertSqlSafe;
use trapperkeeper_server::{Keyring, Kind};

/// A login with the superadmin's tenant and name in any case returns an
/// access token of an hour, a refresh token of a week and the must-change
/// flag, in the body and in cookies marked `HttpOnly`, `SameSite=Strict` and
/// `Secure`. Each token works through the header and through its cookie
/// alone, and a request without a token gets 401 with
/// `WWW-Authenticate: Bearer`. Four kinds of wrong credentials get the same
/// refusal.
#[test]
fn login_then_me() {
    // The test logs the superadmin in, uses both tokens through each channel
    // that a client uses, and tries wrong credentials.
    //
    // 1. We log in in upper case, which login lowercases (ADR-0020).
    // 2. We check the body and the two cookies. 3600 and 604800 are the hour
    //    and the week that the tokens live, and the flag is set, because the
    //    superadmin still holds its generated password.
    // 3. We read `me` with the header, as the CLI does, and with the cookie
    //    alone, as the browser does.
    // 4. We refresh with the refresh cookie alone, as the browser does.
    // 5. We read `me` with no token, which gets the 401 that asks for a
    //    bearer token (RFC 6750, 3).
    // 6. We create a second tenant, so the superadmin's credentials can be
    //    tried under another tenant that exists, and try four wrong logins.
    //    All four get the same body, so a response does not say which part
    //    was wrong.
    on_each_backend(|backend| {
        let server = serve_in_process(backend);

        // 1. log in in upper case
        let response = post_login(
            &server,
            &SYSTEM_TENANT.to_uppercase(),
            &ADMIN.to_uppercase(),
            &server.admin_password,
        );
        assert_eq!(response.status, 200, "{}", response.body);

        // 2. check the body and the cookies
        let login: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(login["token_type"], "Bearer");
        assert_eq!(login["expires_in"], 3600);
        assert_eq!(login["refresh_expires_in"], 7 * 24 * 3600);
        assert_eq!(login["password_must_change"], true);
        let token = login["access_token"].as_str().expect("an access token");
        let refresh = login["refresh_token"].as_str().expect("a refresh token");
        // A signed JWT in compact form has three base64url parts.
        assert_eq!(token.split('.').count(), 3);
        assert_eq!(refresh.split('.').count(), 3);
        let set_cookie = response.header_values("set-cookie");
        assert_eq!(set_cookie.len(), 2, "{set_cookie:?}");
        assert_token_cookie(
            &set_cookie,
            &format!("tk_access={token}"),
            "Path=/api/v1",
            "Max-Age=3600",
        );
        assert_token_cookie(
            &set_cookie,
            &format!("tk_refresh={refresh}"),
            "Path=/api/v1/refresh",
            "Max-Age=604800",
        );

        // 3. read me with the header, then the cookie
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

        // 4. refresh with the cookie
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

        // 5. read me with no token
        let response = me(&server, &[]);
        assert_eq!(response.status, 401);
        assert_eq!(response.header_values("www-authenticate"), ["Bearer"]);

        // 6. create a second tenant, then try four logins
        let other = json!({ "name": "other", "title": "Other", "admin": "boss@other" });
        let response = call(&server, token, "POST", "/api/v1/tenants", Some(&other));
        assert_eq!(response.status, 201, "{}", response.body);
        let admin_password = server.admin_password.as_str();
        for (tenant, name, password) in [
            ("nowhere", ADMIN, admin_password),
            ("other", ADMIN, admin_password),
            (SYSTEM_TENANT, ADMIN, "not the password"),
            (SYSTEM_TENANT, "nobody", admin_password),
        ] {
            let response = post_login(&server, tenant, name, password);
            assert_eq!(response.status, 401);
            assert_eq!(response.body, "unauthorized");
        }
    });
}

/// Logs in to `tenant` as `name` with `password` and returns the response.
fn post_login(
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

/// Sends `GET /api/v1/me` with `headers` and returns the response, whatever
/// its status.
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
    /// Issued two hours ago, past its one-hour lifetime and the 60 seconds of
    /// leeway that the check allows for clock skew.
    Expired,
    /// The user logged out after the token was issued, which revokes every
    /// earlier token.
    BeforeCutoff,
    /// The user was deleted after the token was issued.
    DeletedUser,
}

/// Every variant of `Defect`. The property draws its subsets from this list,
/// so a variant left out is never tested.
const DEFECTS: [Defect; 5] = [
    Defect::FlippedSignature,
    Defect::UnknownKey,
    Defect::Expired,
    Defect::BeforeCutoff,
    Defect::DeletedUser,
];

/// `token` with the last character of its signature changed to another that
/// is valid there, so the signature still decodes and the server refuses the
/// token because the HMAC does not match.
fn flip_signature(token: &str) -> String {
    let (head, signature) = token.rsplit_once('.').expect("a signed token");
    let mut flipped = signature.to_owned();
    let last = flipped.pop().expect("a signature");
    // The last of the 43 characters of an HS256 signature carries 4 data bits
    // and 2 zero bits. `A` and `E` both end in zero bits and differ in a data
    // bit.
    let other = if last == 'A' { 'E' } else { 'A' };
    flipped.push(other);
    format!("{head}.{flipped}")
}

/// `flip_signature` puts a last character in the signature that differs from
/// the old one and has its two low bits zero, as the last of 43 base64url
/// characters must.
#[test]
fn flip_signature_keeps_a_valid_last_character() {
    // `A` takes the branch that writes `E`, and `w` the other branch.
    for last in ['A', 'w'] {
        let token = format!("h.p.{}{last}", "a".repeat(42));
        let new = flip_signature(&token).chars().last().expect("a character");
        assert_ne!(new, last);
        // The base64url characters whose values are multiples of 4.
        assert!("AEIMQUYcgkosw048".contains(new), "{new:?} after {last:?}");
    }
}

/// The user id of the superadmin `ADMIN` of the system tenant, read from the
/// server's database. Panics when the read fails.
fn admin_id(database: &common::TestDatabase) -> String {
    let select = placeholders(
        database.backend,
        "SELECT user_id FROM users WHERE tenant_id = $1 AND user_name = $2",
    );
    on_pool!(database, |pool| {
        // The statement is a literal, so no outside text reaches the SQL.
        sqlx::query_scalar(AssertSqlSafe(select))
            .bind(SYSTEM_TENANT_ID)
            .bind(ADMIN)
            .fetch_one(&pool)
            .await
            .expect("the admin is read")
    })
}

/// Sets `deleted_at` of the superadmin's row to `at`, in epoch microseconds.
/// The test writes the row itself, because no route reaches the superadmin of
/// the system tenant (ADR-0028).
fn delete_admin(database: &common::TestDatabase, at: i64) {
    let update = placeholders(
        database.backend,
        "UPDATE users SET deleted_at = $1 WHERE user_id = $2",
    );
    let id = admin_id(database);
    on_pool!(database, |pool| {
        // The statement is a literal, so no outside text reaches the SQL.
        sqlx::query(AssertSqlSafe(update))
            .bind(at)
            .bind(&id)
            .execute(&pool)
            .await
            .expect("the admin is updated");
    });
}

proptest! {
    #![proptest_config(ProptestConfig {
        // The property runs 12 cases, because each serves a router on every
        // backend and runs Argon2id six times or more.
        cases: 12,
        // No regression file is left in the tree. The panic message shows the
        // minimal failing input.
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// A token passes only when its signature, key, expiry, user and cutoff
    /// are all valid, so `me` answers 200 for a token without a defect and 401
    /// for one with any. A ring that holds the server's key as its previous
    /// key still accepts the tokens that key signed, and a ring without it
    /// refuses them.
    #[test]
    fn a_token_passes_only_when_every_check_holds(
        // Any subset of the five defects, from none to all.
        defects in proptest::sample::subsequence(DEFECTS.to_vec(), 0..=DEFECTS.len())
    ) {
        // The test mints a token with the drawn defects, applies the user
        // defects to the admin's row, and checks the token on the server and
        // on two rings.
        //
        // 1. We serve a router and derive three rings: the server's own, one
        //    of another passphrase, and one that signs with the other and
        //    still checks with the server's key.
        // 2. We issue the token with its token defects.
        // 3. We apply the user defects, which the server sees at the next
        //    request, because it reads the user each time. The logout comes
        //    before the deletion, because a deleted user cannot log in.
        // 4. We send the token to `me`, which must answer 200 only without a
        //    defect.
        // 5. We check the token on the rotated and the other ring, which read
        //    no user row, so only the signature, the key and the expiry
        //    decide. Only the rings show a rotation, because the server's ring
        //    holds no previous key.
        let has = |defect: Defect| defects.contains(&defect);
        for backend in backends() {
            // 1. serve and derive three rings
            let server = serve_in_process(backend);
            let user_id = admin_id(server.database());
            let (own, other, rotated) = block_on(async {
                let store = &server.store;
                (
                    Keyring::from_store(store, TEST_KEY, None).await.expect("the keys derive"),
                    Keyring::from_store(store, TEST_PREVIOUS_KEY, None).await.expect("the keys derive"),
                    Keyring::from_store(store, TEST_PREVIOUS_KEY, Some(TEST_KEY)).await.expect("the keys derive"),
                )
            });

            // 2. issue the token
            let now = SystemTime::now();
            let issued = if has(Defect::Expired) { now - Duration::from_hours(2) } else { now };
            let signer = if has(Defect::UnknownKey) { &other } else { &own };
            let mut token = signer.issue(Kind::Access, &user_id, SYSTEM_TENANT_ID, issued);
            if has(Defect::FlippedSignature) {
                token = flip_signature(&token);
            }
            // 3. apply the user defects
            let now_micros = i64::try_from(
                now.duration_since(UNIX_EPOCH).expect("the clock is past 1970").as_micros(),
            )
            .expect("a small number");
            if has(Defect::BeforeCutoff) {
                let response = call(&server, &superadmin_token(&server), "POST", "/api/v1/logout", None);
                prop_assert_eq!(response.status, 200, "{:?} on {}: {}", defects, backend, response.body);
            }
            if has(Defect::DeletedUser) {
                // The server checks only that `deleted_at` is set.
                delete_admin(server.database(), now_micros);
            }

            // 4. send the token to me
            let bearer = format!("Bearer {token}");
            let response = request(server.addr, "GET", "/api/v1/me", &[("Authorization", &bearer)], None);
            let expected = if defects.is_empty() { 200 } else { 401 };
            prop_assert_eq!(response.status, expected, "{:?} on {}: {}", defects, backend, response.body);

            // 5. check the token on two rings
            let intact = !has(Defect::FlippedSignature) && !has(Defect::Expired);
            // The rotated ring holds both keys, so it accepts an intact token
            // of either signer.
            prop_assert_eq!(rotated.check(Kind::Access, &token).is_ok(), intact, "{:?} on {}", defects, backend);
            // The other ring holds its own key alone.
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
    /// The epoch the token was issued in. The epoch counts the moves of the
    /// admin's cutoff that the model has seen. A logout or a password change
    /// moves the cutoff, which revokes every earlier token.
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
    /// Mint a token of the chosen kind issued eight days ago, past even the
    /// refresh token's week and the leeway.
    Expired { refresh: bool },
}

fn op() -> impl Strategy<Value = Op> {
    // The weights are a choice.
    prop_oneof![
        3 => any::<bool>().prop_map(|right_password| Op::Login { right_password }),
        3 => any::<usize>().prop_map(|token| Op::Me { token }),
        2 => any::<usize>().prop_map(|token| Op::Refresh { token }),
        1 => any::<usize>().prop_map(|token| Op::Logout { token }),
        // Lengths on both sides of the server's minimum of 12.
        1 => (any::<usize>(), any::<bool>(), 8_usize..=20).prop_map(|(token, right_current, new_len)| {
            Op::ChangePassword { token, right_current, new_len }
        }),
        1 => any::<bool>().prop_map(|refresh| Op::Expired { refresh }),
    ]
}

/// The model predicts the server's status for each request and the
/// must-change flag that `me` reports. It holds the admin's password, that
/// flag, the current epoch and every held token.
struct Model {
    password: String,
    must_change: bool,
    epoch: u32,
    held: Vec<Held>,
}

impl Model {
    /// The held token that `index` picks modulo the number held, or a garbage
    /// token when none is held.
    fn token(&self, index: usize) -> Held {
        self.held
            // With no token held, `max(1)` avoids a division by zero, and
            // `get` returns `None`.
            .get(index % self.held.len().max(1))
            .cloned()
            .unwrap_or(Held {
                value: "not.a.token".to_owned(),
                kind: Kind::Access,
                epoch: 0,
                // The token is live for no kind and no epoch, so every route
                // refuses it.
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
        // The property runs 12 cases, because each case serves a router on
        // every backend, which hashes a password and derives a ring, and the
        // case derives one more.
        cases: 12,
        // No regression file is left in the tree.
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// For any sequence of logins, refreshes, logouts, password changes, reads
    /// of `me` and minted expired tokens, each route accepts a token exactly
    /// when the model says it is live.
    #[test]
    // The cap of 8 ops is a choice.
    fn tokens_follow_the_model(ops in proptest::collection::vec(op(), 0..=8)) {
        for backend in backends() {
            let server = serve_in_process(backend);
            let user_id = admin_id(server.database());
            // The ring derives as the server's does, so the tokens it mints
            // fail on their expiry alone.
            let own = block_on(Keyring::from_store(&server.store, TEST_KEY, None))
                .expect("the keys derive");
            let mut model = Model {
                password: server.admin_password.clone(),
                // The superadmin starts with a generated password.
                must_change: true,
                epoch: 0,
                held: Vec::new(),
            };
            // Each op expects the status that the model predicts, and on
            // success the model records what changed.
            for op in &ops {
                match op {
                    Op::Login { right_password } => {
                        let password = if *right_password { model.password.clone() } else { "wrong".to_owned() };
                        let response = post_login(&server, SYSTEM_TENANT, ADMIN, &password);
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
                        // 401 wins over 400, because the token and the
                        // current password are checked before the length. 12
                        // is the server's minimum length.
                        let expected = if !model.live(&held, Kind::Access) || !*right_current {
                            401
                        } else if *new_len < 12 {
                            400
                        } else {
                            200
                        };
                        prop_assert_eq!(response.status, expected, "{:?} {:?} on {}: {}", op, held, backend, response.body);
                        if response.status == 200 {
                            // The server issues the new tokens after it moves
                            // the cutoff, so they belong to the next epoch.
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
                            // A refresh hands out an access token and keeps
                            // the refresh token.
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
                            // A logout clears both cookies with `Max-Age=0`.
                            let set_cookie = response.header_values("set-cookie");
                            prop_assert_eq!(set_cookie.len(), 2, "{:?}", set_cookie);
                            for header in &set_cookie {
                                prop_assert!(header.contains("Max-Age=0"), "{header:?}");
                            }
                            // A logout moves the cutoff, so every token held
                            // before it dies.
                            model.epoch += 1;
                        }
                    }
                    Op::Expired { refresh } => {
                        let kind = if *refresh { Kind::Refresh } else { Kind::Access };
                        // Past the refresh token's week and the leeway.
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
