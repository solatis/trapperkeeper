use actix_web::dev::ServiceResponse;
use actix_web::{http::StatusCode, test, App};
use more_asserts as ma;
use rstest::*;
use serde::Serialize;
use trapperkeeper::web::add_routes;
use trapperkeeper::web::add_state;

use trapperkeeper::models;

pub async fn test_get(route: &String) -> ServiceResponse {
    let mut app = test::init_service(App::new().configure(add_state).configure(add_routes)).await;

    test::call_service(&mut app, test::TestRequest::get().uri(route).to_request()).await
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
    let app_in = app.await;
    let uri = format!("/api/v1/app/{}", app_in.id.unwrap());

    let get_response = test_get(&uri).await;
    let app_out: models::App = test::read_body_json(get_response).await;

    assert_eq!(app_in, app_out);
}
