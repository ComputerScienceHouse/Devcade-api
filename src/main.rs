use actix_cors::Cors;
use actix_web::{http, middleware::Logger, App, HttpServer};

use devcade_api_rs::app::{configure_app, get_app_data};

use std::env;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    let app_data = get_app_data().await;
    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin(&env::var("DOMAIN").unwrap())
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE"])
            .allowed_headers(vec![http::header::AUTHORIZATION, http::header::ACCEPT])
            .allowed_header(http::header::CONTENT_TYPE)
            .max_age(3600);
        App::new()
            .wrap(cors)
            .wrap(Logger::new(
                "%a \"%r\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T",
            ))
            .configure(configure_app)
            .app_data(app_data.clone())
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
