use actix_web::dev::ServiceResponse;
use actix_web::{http::StatusCode, test, App};
use more_asserts as ma;
use rstest::*;
use serde::de::DeserializeOwned;
use serde::Serialize;

use trapperkeeper::models;
use trapperkeeper::utils;
use trapperkeeper::web::add_database;
use trapperkeeper::web::add_routes;

#[cfg(test)]
#[ctor::ctor]
fn init() {
    let _ = env_logger::builder().is_test(true).try_init();
}

fn gen_identifier() -> String {
    utils::random_token()
}

fn gen_app_name() -> String {
    gen_identifier()
}

fn gen_auth_token_name() -> String {
    gen_identifier()
}

pub async fn test_get(route: &String) -> ServiceResponse {
    let mut app =
        test::init_service(App::new().configure(add_database).configure(add_routes)).await;

    test::call_service(&mut app, test::TestRequest::get().uri(route).to_request()).await
}

pub async fn test_get_json<T>(route: &String) -> T
where
    T: DeserializeOwned,
{
    let resp = test_get(route).await;
    assert!(resp.status().is_success());

    test::read_body_json(resp).await
}

pub async fn test_delete(route: &String) -> ServiceResponse {
    let mut app =
        test::init_service(App::new().configure(add_database).configure(add_routes)).await;

    test::call_service(
        &mut app,
        test::TestRequest::delete().uri(route).to_request(),
    )
    .await
}

pub async fn test_post<T>(route: &str, params: &T) -> ServiceResponse
where
    T: Serialize,
{
    let mut app =
        test::init_service(App::new().configure(add_database).configure(add_routes)).await;

    test::call_service(
        &mut app,
        test::TestRequest::post()
            .set_json(&params)
            .uri(route)
            .to_request(),
    )
    .await
}

pub async fn test_post_json<T, D>(route: &str, params: &T) -> D
where
    T: Serialize,
    D: DeserializeOwned,
{
    let resp = test_post(route, params).await;
    assert!(resp.status().is_success());

    test::read_body_json(resp).await
}

#[fixture]
pub fn new_app() -> models::NewApp {
    models::NewApp::new(&gen_app_name())
}

#[fixture]
pub async fn app(new_app: models::NewApp) -> models::App {
    let resp = test_post("/api/v1/app", &new_app).await;
    assert_eq!(resp.status(), StatusCode::OK);

    test::read_body_json(resp).await
}

#[fixture]
pub async fn new_auth_token(#[future] app: models::App) -> models::NewAuthToken {
    let app_: models::App = app.await;
    models::NewAuthToken::new(app_.id.unwrap(), &gen_auth_token_name())
}

#[fixture]
pub async fn auth_token(#[future] new_auth_token: models::NewAuthToken) -> models::AuthToken {
    let new_auth_token = new_auth_token.await;
    let uri = format!("/api/v1/app/{}/auth_token", new_auth_token.app_id);

    test_post_json(&uri, &new_auth_token).await
}

#[rstest]
#[actix_web::test]
async fn test_app_create(new_app: models::NewApp) {
    let resp = test_post("/api/v1/app", &new_app).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let app: models::App = test::read_body_json(resp).await;

    assert_eq!(app.name, new_app.name);
    ma::assert_gt!(app.id, Some(0))
}

#[rstest]
#[actix_web::test]
async fn test_app_get(#[future] app: models::App) {
    let app = app.await;
    let uri = format!("/api/v1/app/{}", app.id.unwrap());

    let app_: models::App = test_get_json(&uri).await;

    assert_eq!(app, app_);
}

#[rstest]
#[actix_web::test]
async fn test_app_get_nonexisting_app() {
    let resp = test_get(&String::from("/api/v1/app/0")).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[rstest]
#[actix_web::test]
async fn test_app_delete(#[future] app: models::App) {
    let app = app.await;
    let uri = format!("/api/v1/app/{}", app.id.unwrap());

    // Get before delete
    let resp = test_get(&uri).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Actual delete
    let resp = test_delete(&uri).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Get after delete
    let resp = test_get(&uri).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Delete after delete
    let resp = test_delete(&uri).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[rstest]
#[actix_web::test]
async fn test_auth_token_create(#[future] new_auth_token: models::NewAuthToken) {
    let new_auth_token = new_auth_token.await;
    let uri = format!("/api/v1/app/{}/auth_token", new_auth_token.app_id);

    let auth_token: models::AuthToken = test_post_json(&uri, &new_auth_token).await;

    assert_eq!(auth_token.name, new_auth_token.name);
    assert_eq!(auth_token.app_id, new_auth_token.app_id);
}

#[rstest]
#[actix_web::test]
async fn test_auth_token_create_incorrect_app_id(#[future] new_auth_token: models::NewAuthToken) {
    let new_auth_token = new_auth_token.await;
    let uri = String::from("/api/v1/app/1/auth_token");

    let resp = test_post(&uri, &new_auth_token).await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[rstest]
#[actix_web::test]
async fn test_auth_token_get(#[future] auth_token: models::AuthToken) {
    let auth_token = auth_token.await;

    let uri1 = format!(
        "/api/v1/app/{}/auth_token/{}",
        auth_token.app_id, auth_token.id
    );

    let uri2 = format!("/api/v1/auth_token/{}", auth_token.id);

    let auth_token1: models::AuthToken = test_get_json(&uri1).await;
    let auth_token2: models::AuthToken = test_get_json(&uri2).await;

    assert_eq!(auth_token, auth_token1);
    assert_eq!(auth_token, auth_token2);
}

#[rstest]
#[actix_web::test]
async fn test_auth_token_delete_by_app(#[future] auth_token: models::AuthToken) {
    let auth_token = auth_token.await;

    let uri = format!(
        "/api/v1/app/{}/auth_token/{}",
        auth_token.app_id, auth_token.id
    );

    let resp = test_get(&uri).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = test_delete(&uri).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = test_get(&uri).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let resp = test_delete(&uri).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[rstest]
#[actix_web::test]
async fn test_auth_token_delete(#[future] auth_token: models::AuthToken) {
    let auth_token = auth_token.await;

    let uri = format!("/api/v1/auth_token/{}", auth_token.id);

    let resp = test_get(&uri).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = test_delete(&uri).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = test_get(&uri).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let resp = test_delete(&uri).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
