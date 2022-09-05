use actix_web::dev::ServiceResponse;
use actix_web::{http::StatusCode, test, App};
use more_asserts as ma;
use rstest::*;
use serde::de::DeserializeOwned;
use serde::Serialize;
use trapperkeeper::web::add_routes;
use trapperkeeper::web::add_state;

use trapperkeeper::models;

pub async fn test_get(route: &String) -> ServiceResponse {
    let mut app = test::init_service(App::new().configure(add_state).configure(add_routes)).await;

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
    let mut app = test::init_service(App::new().configure(add_state).configure(add_routes)).await;

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
    let mut app = test::init_service(App::new().configure(add_state).configure(add_routes)).await;

    test::call_service(
        &mut app,
        test::TestRequest::post()
            .set_json(&params)
            .uri(route)
            .to_request(),
    )
    .await
}

#[fixture]
pub fn new_app() -> models::NewApp {
    models::NewApp::new("foo")
}

#[fixture]
pub async fn app() -> models::App {
    let new_app = models::NewApp::new("foo");
    let resp = test_post("/api/v1/app", &new_app).await;
    assert_eq!(resp.status(), StatusCode::OK);

    test::read_body_json(resp).await
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
