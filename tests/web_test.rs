use actix_web::dev::ServiceResponse;
use actix_web::{http::StatusCode, test, App};
use more_asserts as ma;
use rstest::*;
use serde::Serialize;
use trapperkeeper::web::add_routes;
use trapperkeeper::web::add_state;

use trapperkeeper::models;

pub async fn test_get(route: &str) -> ServiceResponse {
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
pub fn app() -> models::NewApp {
    models::NewApp::new("foo")
}

#[rstest]
#[actix_web::test]
async fn test_app_create(app: models::NewApp) {
    let resp = test_post("/api/v1/app", &app).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let app_: models::App = test::read_body_json(resp).await;

    assert_eq!(app_.name, app.name);
    ma::assert_gt!(app_.id, Some(0))
}
