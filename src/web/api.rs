use actix_web::{web, Error, HttpResponse};

use crate::crud;
use crate::database;
use crate::models;

fn unwrap_get_result<T>(result: Result<Option<T>, crud::Error>) -> Result<HttpResponse, Error>
where
    T: serde::Serialize,
{
    let result_ = result.map_err(actix_web::error::ErrorInternalServerError)?;

    match result_ {
        Some(x) => Ok(HttpResponse::Ok().json(x)),
        None => Ok(HttpResponse::NotFound().finish()),
    }
}

fn unwrap_delete_result(result: Result<bool, crud::Error>) -> Result<HttpResponse, Error> {
    let result_ = result.map_err(actix_web::error::ErrorInternalServerError)?;

    match result_ {
        true => Ok(HttpResponse::Ok().finish()),
        false => Ok(HttpResponse::NotFound().finish()),
    }
}

async fn create_trapp(
    mut conn: database::PooledConnection,
    trapp: web::Json<models::NewTrapp>,
) -> Result<HttpResponse, Error> {
    log::info!("create_trapp");

    let trapp = web::block(move || crud::create_trapp(conn.as_mut(), &trapp.name))
        .await?
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(trapp))
}

async fn get_trapp(
    mut conn: database::PooledConnection,
    trapp_id: web::Path<i32>,
) -> Result<HttpResponse, Error> {
    log::info!("get_trapp");

    let trapp_id = trapp_id.into_inner();

    let result = web::block(move || crud::get_trapp_by_id(conn.as_mut(), trapp_id)).await?;

    unwrap_get_result(result)
}

async fn delete_trapp(
    mut conn: database::PooledConnection,
    trapp_id: web::Path<i32>,
) -> Result<HttpResponse, Error> {
    log::info!("delete_trapp");

    let trapp_id = trapp_id.into_inner();

    let result = web::block(move || crud::delete_trapp_by_id(conn.as_mut(), trapp_id)).await?;

    unwrap_delete_result(result)
}

async fn create_auth_token(
    mut conn: database::PooledConnection,
    trapp_id: web::Path<i32>,
    auth_token: web::Json<models::NewAuthToken>,
) -> Result<HttpResponse, Error> {
    log::info!("create_auth_token");

    if trapp_id.into_inner() != auth_token.trapp_id {
        return Ok(
            HttpResponse::BadRequest().body("trapp_id in auth_token must match trapp_id in uri")
        );
    }

    let auth_token = web::block(move || {
        crud::create_auth_token(conn.as_mut(), auth_token.trapp_id, &auth_token.name)
    })
    .await?
    .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(auth_token))
}

async fn get_trapp_auth_token(
    mut conn: database::PooledConnection,
    path: web::Path<(i32, String)>,
) -> Result<HttpResponse, Error> {
    log::info!("get_trapp_auth_token");

    let (trapp_id, auth_token_id) = path.into_inner();

    let result = web::block(move || {
        crud::get_auth_token_by_trapp_and_id(conn.as_mut(), trapp_id, &auth_token_id)
    })
    .await?;

    unwrap_get_result(result)
}

async fn delete_trapp_auth_token(
    mut conn: database::PooledConnection,
    path: web::Path<(i32, String)>,
) -> Result<HttpResponse, Error> {
    log::info!("delete_trapp_auth_token");

    let (trapp_id, auth_token_id) = path.into_inner();

    let result = web::block(move || {
        crud::delete_auth_token_by_trapp_and_id(conn.as_mut(), trapp_id, &auth_token_id)
    })
    .await?;

    unwrap_delete_result(result)
}

async fn delete_auth_token(
    mut conn: database::PooledConnection,
    path: web::Path<String>,
) -> Result<HttpResponse, Error> {
    log::info!("delete_trapp_auth_token");

    let auth_token_id = path.into_inner();

    let result =
        web::block(move || crud::delete_auth_token_by_id(conn.as_mut(), &auth_token_id)).await?;

    unwrap_delete_result(result)
}

async fn get_auth_token(
    mut conn: database::PooledConnection,
    path: web::Path<String>,
) -> Result<HttpResponse, Error> {
    log::info!("get_auth_token");

    let auth_token_id = path.into_inner();

    let result =
        web::block(move || crud::get_auth_token_by_id(conn.as_mut(), &auth_token_id)).await?;

    unwrap_get_result(result)
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .route("/trapp", web::post().to(create_trapp))
            .route("/trapp/{trapp_id}", web::get().to(get_trapp))
            .route("/trapp/{trapp_id}", web::delete().to(delete_trapp))
            .route(
                "/trapp/{trapp_id}/auth_token",
                web::post().to(create_auth_token),
            )
            .route(
                "/trapp/{trapp_id}/auth_token/{auth_token_id}",
                web::get().to(get_trapp_auth_token),
            )
            .route(
                "/trapp/{trapp_id}/auth_token/{auth_token_id}",
                web::delete().to(delete_trapp_auth_token),
            )
            .route("/auth_token/{auth_token_id}", web::get().to(get_auth_token))
            .route(
                "/auth_token/{auth_token_id}",
                web::delete().to(delete_auth_token),
            ),
    );
}
