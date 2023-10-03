use crate::{
    games::routes::{self as games, FileUploadDoc, GameData, GameUploadDoc},
    models::{AppState, Game, GameWithTags, Tag, User, UserType},
    tags::routes as tags,
    users::routes as users,
};

use actix_web::web::{self, scope, Data};
use aws_sdk_s3 as s3;
use aws_sdk_s3::Endpoint;

use sqlx::postgres::PgPoolOptions;
use std::env;
use utoipa::{
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
    Modify, OpenApi,
};
use utoipa_swagger_ui::SwaggerUi;

pub fn configure_app(cfg: &mut web::ServiceConfig) {
    #[derive(OpenApi)]
    #[openapi(
        paths(
            games::get_all_games,
            games::get_game,
            games::edit_game,
            games::delete_game,
            games::add_game,
            games::get_binary,
            games::update_binary,
            games::get_banner,
            games::update_banner,
            games::get_icon,
            games::update_icon,
            tags::get_all_tags,
            tags::get_tag,
            tags::edit_tag,
            tags::delete_tag,
            tags::add_tag,
            tags::get_tag_games,
            users::get_user,
            users::add_user,
            users::edit_user,
        ),
        components(
            schemas(GameData, Game, GameUploadDoc, FileUploadDoc, GameWithTags, Tag, User, UserType)
        ),
        tags(
            (name = "DevcadeAPI", description = "")
        ),
        modifiers(&SecurityAddon)
    )]
    struct ApiDoc;

    struct SecurityAddon;

    impl Modify for SecurityAddon {
        fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
            let components = openapi.components.as_mut().unwrap(); // we can unwrap safely since there already is components registered.
            components.add_security_scheme(
                "api_key",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("frontend_api_key"))),
            )
        }
    }

    let openapi = ApiDoc::openapi();
    cfg.service(
        scope("/api")
            .service(
                scope("/games")
                    .service(games::get_all_games)
                    .service(games::get_game)
                    .service(games::edit_game)
                    .service(games::delete_game)
                    .service(games::add_game)
                    .service(games::get_binary)
                    .service(games::update_binary)
                    .service(games::get_banner)
                    .service(games::update_banner)
                    .service(games::get_icon)
                    .service(games::update_icon),
            )
            .service(
                scope("/tags")
                    .service(tags::get_all_tags)
                    .service(tags::get_tag)
                    .service(tags::edit_tag)
                    .service(tags::delete_tag)
                    .service(tags::add_tag)
                    .service(tags::get_tag_games),
            )
            .service(
                scope("/users")
                    .service(users::get_user)
                    .service(users::add_user)
                    .service(users::edit_user),
            )
            .service(SwaggerUi::new("/docs/{_:.*}").url("/api/api-doc/openapi.json", openapi)),
    );
}

pub async fn get_app_data() -> Data<AppState> {
    let shared_config = aws_config::load_from_env().await;

    // Create an S3 config from the shared config and override the endpoint resolver.
    let s3_config = s3::config::Builder::from(&shared_config)
        .endpoint_resolver(Endpoint::immutable(
            env::var("S3_ENDPOINT")
                .unwrap_or("https://s3.csh.rit.edu".to_string())
                .parse()
                .unwrap(),
        ))
        //.endpoint_resolver(s3::Endpoint::immutable("https://s3.csh.rit.edu".parse().unwrap()))
        .build();
    let s3_conn = s3::Client::from_conf(s3_config);

    let pool = PgPoolOptions::new()
        .connect(&env::var("SQL_URI").unwrap())
        .await
        .unwrap();
    Data::new(AppState {
        db: pool,
        s3: s3_conn.clone(),
    })
}
