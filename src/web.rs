use actix_web::{get, post, web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder};

use crate::crud;
use crate::database;
use crate::models;

struct State {
    db_pool: database::DbPool,
}

async fn create_app(
    s: web::Data<State>,
    app: web::Json<models::NewApp>,
) -> Result<HttpResponse, Error> {
    let mut conn = s
        .db_pool
        .get()
        .expect("could not get database connection from pool");

    let app = web::block(move || crud::create_app(&mut conn, &app.name))
        .await?
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(app))
}

async fn get_app(s: web::Data<State>, app_id: web::Path<i32>) -> Result<HttpResponse, Error> {
    let mut conn = s
        .db_pool
        .get()
        .expect("could not get database connection from pool");

    let app_id = app_id.into_inner();
    let app = web::block(move || crud::get_app_by_id(&mut conn, app_id))
        .await?
        .map_err(actix_web::error::ErrorInternalServerError)?;

    match app {
        Some(x) => Ok(HttpResponse::Ok().json(x)),
        None => Ok(HttpResponse::NotFound().finish()),
    }
}

async fn delete_app(s: web::Data<State>, app_id: web::Path<i32>) -> Result<HttpResponse, Error> {
    let mut conn = s
        .db_pool
        .get()
        .expect("could not get database connection from pool");

    let app_id = app_id.into_inner();
    let result = web::block(move || crud::delete_app_by_id(&mut conn, app_id))
        .await?
        .map_err(actix_web::error::ErrorInternalServerError)?;

    match result {
        true => Ok(HttpResponse::Ok().finish()),
        false => Ok(HttpResponse::NotFound().finish()),
    }
}

pub fn add_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .route("/app", web::post().to(create_app))
            .route("/app/{app_id}", web::get().to(get_app))
            .route("/app/{app_id}", web::delete().to(delete_app)),
    );
}

pub fn add_state(cfg: &mut web::ServiceConfig) {
    let pool = database::pool();
    database::run_migrations(&mut pool.get().unwrap()).expect("Unable to run migrations");

    cfg.app_data(web::Data::new(State {
        db_pool: pool.clone(),
    }));
}

#[actix_web::main]
pub async fn run() -> std::io::Result<()> {
    HttpServer::new(move || App::new().configure(add_state).configure(add_routes))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
