use actix_web::{web, Error, HttpResponse};

use crate::crud;
use crate::database;
use crate::models;

fn unwrap_get_result<T>(
    result: Result<Option<T>, diesel::result::Error>,
) -> Result<HttpResponse, Error>
where
    T: serde::Serialize,
{
    let result_ = result.map_err(actix_web::error::ErrorInternalServerError)?;

    match result_ {
        Some(x) => Ok(HttpResponse::Ok().json(x)),
        None => Ok(HttpResponse::NotFound().finish()),
    }
}

fn unwrap_delete_result(
    result: Result<bool, diesel::result::Error>,
) -> Result<HttpResponse, Error> {
    let result_ = result.map_err(actix_web::error::ErrorInternalServerError)?;

    match result_ {
        true => Ok(HttpResponse::Ok().finish()),
        false => Ok(HttpResponse::NotFound().finish()),
    }
}

fn get_conn() -> Result<database::PooledConnection, Error> {
    database::POOL
        .get()
        .map_err(actix_web::error::ErrorInternalServerError)
}

async fn create_app(app: web::Json<models::NewApp>) -> Result<HttpResponse, Error> {
    log::info!("create_app");

    let mut conn = get_conn()?;
    let app = web::block(move || crud::create_app(&mut conn, &app.name))
        .await?
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(app))
}

async fn get_app(app_id: web::Path<i32>) -> Result<HttpResponse, Error> {
    log::info!("get_app");

    let app_id = app_id.into_inner();

    let mut conn = get_conn()?;
    let result = web::block(move || crud::get_app_by_id(&mut conn, app_id)).await?;

    unwrap_get_result(result)
}

async fn delete_app(app_id: web::Path<i32>) -> Result<HttpResponse, Error> {
    log::info!("delete_app");

    let app_id = app_id.into_inner();

    let mut conn = get_conn()?;
    let result = web::block(move || crud::delete_app_by_id(&mut conn, app_id)).await?;

    unwrap_delete_result(result)
}

async fn create_auth_token(
    app_id: web::Path<i32>,
    auth_token: web::Json<models::NewAuthToken>,
) -> Result<HttpResponse, Error> {
    log::info!("create_auth_token");

    if app_id.into_inner() != auth_token.app_id {
        return Ok(HttpResponse::BadRequest().body("app_id in auth_token must match app_id in uri"));
    }

    let mut conn = get_conn()?;
    let auth_token =
        web::block(move || crud::create_auth_token(&mut conn, auth_token.app_id, &auth_token.name))
            .await?
            .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(auth_token))
}

async fn get_app_auth_token(path: web::Path<(i32, String)>) -> Result<HttpResponse, Error> {
    log::info!("get_app_auth_token");

    let (app_id, auth_token_id) = path.into_inner();

    let mut conn = get_conn()?;
    let result =
        web::block(move || crud::get_auth_token_by_app_and_id(&mut conn, app_id, &auth_token_id))
            .await?;

    unwrap_get_result(result)
}

async fn delete_app_auth_token(path: web::Path<(i32, String)>) -> Result<HttpResponse, Error> {
    log::info!("delete_app_auth_token");

    let (app_id, auth_token_id) = path.into_inner();

    let mut conn = get_conn()?;
    let result = web::block(move || {
        crud::delete_auth_token_by_app_and_id(&mut conn, app_id, &auth_token_id)
    })
    .await?;

    unwrap_delete_result(result)
}

async fn delete_auth_token(path: web::Path<String>) -> Result<HttpResponse, Error> {
    log::info!("delete_app_auth_token");

    let auth_token_id = path.into_inner();

    let mut conn = get_conn()?;
    let result =
        web::block(move || crud::delete_auth_token_by_id(&mut conn, &auth_token_id)).await?;

    unwrap_delete_result(result)
}

async fn get_auth_token(path: web::Path<String>) -> Result<HttpResponse, Error> {
    log::info!("get_auth_token");

    let auth_token_id = path.into_inner();

    let mut conn = get_conn()?;
    let result = web::block(move || crud::get_auth_token_by_id(&mut conn, &auth_token_id)).await?;

    unwrap_get_result(result)
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .route("/app", web::post().to(create_app))
            .route("/app/{app_id}", web::get().to(get_app))
            .route("/app/{app_id}", web::delete().to(delete_app))
            .route(
                "/app/{app_id}/auth_token",
                web::post().to(create_auth_token),
            )
            .route(
                "/app/{app_id}/auth_token/{auth_token_id}",
                web::get().to(get_app_auth_token),
            )
            .route(
                "/app/{app_id}/auth_token/{auth_token_id}",
                web::delete().to(delete_app_auth_token),
            )
            .route("/auth_token/{auth_token_id}", web::get().to(get_auth_token))
            .route(
                "/auth_token/{auth_token_id}",
                web::delete().to(delete_auth_token),
            ),
    );
}
