use actix_web::{cookie, http::StatusCode};
use rstest::*;

use trapperkeeper::config;
use trapperkeeper::crypto;
use trapperkeeper::models;

use super::util;

#[cfg(test)]
#[ctor::ctor]
fn init() {
    let _ = env_logger::builder().is_test(true).try_init();
}

#[rstest]
#[actix_web::test]
async fn test_can_get_login() {
    let resp = util::test_get("/admin/login").await;

    assert_eq!(resp.status(), StatusCode::OK);
}

#[rstest]
#[actix_web::test]
async fn test_valid_login() {
    let credentials = &config::CONFIG.admin;
    let login = models::Login::new(&credentials.username, &credentials.password);
    let resp = util::test_post_form("/admin/login", &login).await;
    let headers = resp.headers();

    // A valid login means we get redirected to the index page
    assert_eq!(resp.status(), StatusCode::FOUND);

    let location = headers
        .get("Location")
        .expect("Location header not set")
        .to_str()
        .expect("Unable to convert location header to string -- not valid ascii?");
    assert_eq!(location, "/admin/index");

    // The part below verifies the set-cookie behavior
    let set_cookie = headers
        .get("Set-Cookie")
        .expect("Set-Cookie header not present")
        .to_str()
        .expect("Unable to convert cookie to str -- not valid ascii?")
        .to_owned();

    // We expect an auth cookie to be present
    let cookie: cookie::Cookie =
        cookie::Cookie::parse(set_cookie).expect("Unable to parse set-cookie");
    assert_eq!(cookie.name(), "authorization");
    assert_eq!(cookie.http_only(), Some(true));

    // And we expect the auth cookie to be a valid JWT token.
    let jwt: String = String::from(cookie.value());

    // Note that in debug mode, the HMAC is deterministic, not random, and as such
    // we can just generate a "random" hmac and we'll be able to parse it.
    let hm = crypto::random_hmac();
    let claim: models::Session = crypto::jwt_decode(&jwt, &hm).expect("Unable to decode JWT");

    // The cream of the crop: the decoded JWT has a valid username set.
    assert_eq!(claim.username, login.username);
}

#[rstest]
#[actix_web::test]
async fn test_invalid_login() {
    let login = models::Login::new(&String::from("invalid"), &String::from("invalid"));
    let resp = util::test_post_form("/admin/login", &login).await;
    let headers = resp.headers();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(headers.contains_key("Set-Cookie"), false);
    assert_eq!(headers.contains_key("Location"), false);
}
