use actix_web::dev::ServiceResponse;
use actix_web::{test, App};

use serde::de::DeserializeOwned;
use serde::Serialize;

use trapperkeeper::web;

pub async fn test_get(route: &String) -> ServiceResponse {
    let mut app = test::init_service(App::new().configure(web::configure)).await;

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
    let mut app = test::init_service(App::new().configure(web::configure)).await;

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
    let mut app = test::init_service(App::new().configure(web::configure)).await;

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
