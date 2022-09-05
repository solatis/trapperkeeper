use actix_web::{web, App, Error, HttpResponse, HttpServer};

use crate::crud;
use crate::database;
use crate::models;

async fn create_app(
    db_pool: web::Data<database::DbPool>,
    app: web::Json<models::NewApp>,
) -> Result<HttpResponse, Error> {
    let mut conn = db_pool
        .get()
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let app = web::block(move || crud::create_app(&mut conn, &app.name))
        .await?
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(app))
}

async fn get_app(
    db_pool: web::Data<database::DbPool>,
    app_id: web::Path<i32>,
) -> Result<HttpResponse, Error> {
    let mut conn = db_pool
        .get()
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let app_id = app_id.into_inner();
    let app = web::block(move || crud::get_app_by_id(&mut conn, app_id))
        .await?
        .map_err(actix_web::error::ErrorInternalServerError)?;

    match app {
        Some(x) => Ok(HttpResponse::Ok().json(x)),
        None => Ok(HttpResponse::NotFound().finish()),
    }
}

async fn delete_app(
    db_pool: web::Data<database::DbPool>,
    app_id: web::Path<i32>,
) -> Result<HttpResponse, Error> {
    let mut conn = db_pool
        .get()
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let app_id = app_id.into_inner();
    let result = web::block(move || crud::delete_app_by_id(&mut conn, app_id))
        .await?
        .map_err(actix_web::error::ErrorInternalServerError)?;

    match result {
        true => Ok(HttpResponse::Ok().finish()),
        false => Ok(HttpResponse::NotFound().finish()),
    }
}

async fn create_auth_token(
    db_pool: web::Data<database::DbPool>,
    app_id: web::Path<i32>,
    auth_token: web::Json<models::NewAuthToken>,
) -> Result<HttpResponse, Error> {
    let mut conn = db_pool
        .get()
        .map_err(actix_web::error::ErrorInternalServerError)?;

    if app_id.into_inner() != auth_token.app_id {
        return Ok(HttpResponse::BadRequest().body("app_id in auth_token must match app_id in uri"));
    }

    let auth_token =
        web::block(move || crud::create_auth_token(&mut conn, auth_token.app_id, &auth_token.name))
            .await?
            .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(auth_token))
}

async fn get_app_auth_token(
    db_pool: web::Data<database::DbPool>,
    path: web::Path<(i32, String)>,
) -> Result<HttpResponse, Error> {
    let mut conn = db_pool
        .get()
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let (app_id, auth_token_id) = path.into_inner();

    let auth_token =
        web::block(move || crud::get_auth_token_by_app_and_id(&mut conn, app_id, &auth_token_id))
            .await?
            .map_err(actix_web::error::ErrorInternalServerError)?;

    match auth_token {
        Some(x) => Ok(HttpResponse::Ok().json(x)),
        None => Ok(HttpResponse::NotFound().finish()),
    }
}

async fn delete_app_auth_token(
    db_pool: web::Data<database::DbPool>,
    path: web::Path<(i32, String)>,
) -> Result<HttpResponse, Error> {
    let mut conn = db_pool
        .get()
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let (app_id, auth_token_id) = path.into_inner();
    let result = web::block(move || {
        crud::delete_auth_token_by_app_and_id(&mut conn, app_id, &auth_token_id)
    })
    .await?
    .map_err(actix_web::error::ErrorInternalServerError)?;

    match result {
        true => Ok(HttpResponse::Ok().finish()),
        false => Ok(HttpResponse::NotFound().finish()),
    }
}

async fn delete_auth_token(
    db_pool: web::Data<database::DbPool>,
    path: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let mut conn = db_pool
        .get()
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let auth_token_id = path.into_inner();

    let result = web::block(move || crud::delete_auth_token_by_id(&mut conn, &auth_token_id))
        .await?
        .map_err(actix_web::error::ErrorInternalServerError)?;

    match result {
        true => Ok(HttpResponse::Ok().finish()),
        false => Ok(HttpResponse::NotFound().finish()),
    }
}

async fn get_auth_token(
    db_pool: web::Data<database::DbPool>,
    path: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let mut conn = db_pool
        .get()
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let auth_token_id = path.into_inner();

    let auth_token = web::block(move || crud::get_auth_token_by_id(&mut conn, &auth_token_id))
        .await?
        .map_err(actix_web::error::ErrorInternalServerError)?;

    match auth_token {
        Some(x) => Ok(HttpResponse::Ok().json(x)),
        None => Ok(HttpResponse::NotFound().finish()),
    }
}

pub fn add_routes(cfg: &mut web::ServiceConfig) {
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

pub fn add_database(cfg: &mut web::ServiceConfig) {
    let pool: database::DbPool = database::pool();
    database::run_migrations(&mut pool.get().unwrap()).expect("Unable to run migrations");

    cfg.app_data(web::Data::new(pool.clone()));
}

#[actix_web::main]
pub async fn run() -> std::io::Result<()> {
    HttpServer::new(move || App::new().configure(add_database).configure(add_routes))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
