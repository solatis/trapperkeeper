use actix_web::{get, post, web, App, Error, HttpResponse, HttpServer, Responder};

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

#[actix_web::main]
pub async fn run(pool: database::DbPool) -> std::io::Result<()> {
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(State {
                db_pool: pool.clone(),
            }))
            .service(web::scope("/api/v1").route("/app", web::post().to(create_app)))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
