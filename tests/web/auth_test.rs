use actix_web::{http::StatusCode, test, App};
use more_asserts as ma;
use rstest::*;

use trapperkeeper::config;
use trapperkeeper::database;
use trapperkeeper::models;
use trapperkeeper::web;

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

    assert_eq!(resp.status(), StatusCode::OK);
}

#[rstest]
#[actix_web::test]
async fn test_invalid_login() {
    let login = models::Login::new(&String::from("invalid"), &String::from("invalid"));
    let resp = util::test_post_form("/admin/login", &login).await;

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
