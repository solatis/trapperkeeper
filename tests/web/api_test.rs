use actix_web::{http::StatusCode, test};
use more_asserts as ma;
use rstest::*;

use trapperkeeper::models;
use trapperkeeper::utils::random_token;

use super::util;

#[cfg(test)]
#[ctor::ctor]
fn init() {
    let _ = env_logger::builder().is_test(true).try_init();
}

fn gen_identifier() -> String {
    random_token(16)
}

fn gen_trapp_name() -> String {
    gen_identifier()
}

fn gen_auth_token_name() -> String {
    gen_identifier()
}

#[fixture]
pub fn new_trapp() -> models::NewTrapp {
    models::NewTrapp::new(&gen_trapp_name())
}

#[fixture]
pub async fn trapp(new_trapp: models::NewTrapp) -> models::Trapp {
    let resp = util::test_post("/api/v1/trapp", &new_trapp).await;
    assert_eq!(resp.status(), StatusCode::OK);

    test::read_body_json(resp).await
}

#[fixture]
pub async fn new_auth_token(#[future] trapp: models::Trapp) -> models::NewAuthToken {
    let trapp_: models::Trapp = trapp.await;
    models::NewAuthToken::new(trapp_.id.unwrap(), &gen_auth_token_name())
}

#[fixture]
pub async fn auth_token(#[future] new_auth_token: models::NewAuthToken) -> models::AuthToken {
    let new_auth_token = new_auth_token.await;
    let uri = format!("/api/v1/trapp/{}/auth_token", new_auth_token.trapp_id);

    util::test_post_json(&uri, &new_auth_token).await
}

#[rstest]
#[actix_web::test]
async fn test_trapp_create(new_trapp: models::NewTrapp) {
    let resp = util::test_post("/api/v1/trapp", &new_trapp).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let trapp: models::Trapp = test::read_body_json(resp).await;

    assert_eq!(trapp.name, new_trapp.name);
    ma::assert_gt!(trapp.id, Some(0))
}

#[rstest]
#[actix_web::test]
async fn test_trapp_get(#[future] trapp: models::Trapp) {
    let trapp = trapp.await;
    let uri = format!("/api/v1/trapp/{}", trapp.id.unwrap());

    let trapp_: models::Trapp = util::test_get_json(&uri).await;

    assert_eq!(trapp, trapp_);
}

#[rstest]
#[actix_web::test]
async fn test_trapp_get_nonexisting_trapp() {
    let resp = util::test_get(&String::from("/api/v1/trapp/0")).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[rstest]
#[actix_web::test]
async fn test_trapp_delete(#[future] trapp: models::Trapp) {
    let trapp = trapp.await;
    let uri = format!("/api/v1/trapp/{}", trapp.id.unwrap());

    // Get before delete
    let resp = util::test_get(&uri).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Actual delete
    let resp = util::test_delete(&uri).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Get after delete
    let resp = util::test_get(&uri).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Delete after delete
    let resp = util::test_delete(&uri).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[rstest]
#[actix_web::test]
async fn test_auth_token_create(#[future] new_auth_token: models::NewAuthToken) {
    let new_auth_token = new_auth_token.await;
    let uri = format!("/api/v1/trapp/{}/auth_token", new_auth_token.trapp_id);

    let auth_token: models::AuthToken = util::test_post_json(&uri, &new_auth_token).await;

    assert_eq!(auth_token.name, new_auth_token.name);
    assert_eq!(auth_token.trapp_id, new_auth_token.trapp_id);
}

#[rstest]
#[actix_web::test]
async fn test_auth_token_create_incorrect_trapp_id(#[future] new_auth_token: models::NewAuthToken) {
    let new_auth_token = new_auth_token.await;
    let uri = String::from("/api/v1/trapp/1/auth_token");

    let resp = util::test_post(&uri, &new_auth_token).await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[rstest]
#[actix_web::test]
async fn test_auth_token_get(#[future] auth_token: models::AuthToken) {
    let auth_token = auth_token.await;

    let uri1 = format!(
        "/api/v1/trapp/{}/auth_token/{}",
        auth_token.trapp_id, auth_token.id
    );

    let uri2 = format!("/api/v1/auth_token/{}", auth_token.id);

    let auth_token1: models::AuthToken = util::test_get_json(&uri1).await;
    let auth_token2: models::AuthToken = util::test_get_json(&uri2).await;

    assert_eq!(auth_token, auth_token1);
    assert_eq!(auth_token, auth_token2);
}

#[rstest]
#[actix_web::test]
async fn test_auth_token_delete_by_trapp(#[future] auth_token: models::AuthToken) {
    let auth_token = auth_token.await;

    let uri = format!(
        "/api/v1/trapp/{}/auth_token/{}",
        auth_token.trapp_id, auth_token.id
    );

    let resp = util::test_get(&uri).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = util::test_delete(&uri).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = util::test_get(&uri).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let resp = util::test_delete(&uri).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[rstest]
#[actix_web::test]
async fn test_auth_token_delete(#[future] auth_token: models::AuthToken) {
    let auth_token = auth_token.await;

    let uri = format!("/api/v1/auth_token/{}", auth_token.id);

    let resp = util::test_get(&uri).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = util::test_delete(&uri).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = util::test_get(&uri).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let resp = util::test_delete(&uri).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
