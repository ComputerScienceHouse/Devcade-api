use crate::{
    games::flatpak::FlatpakFile,
    models::{AppState, Game, GameWithTags},
    security::RequireApiKey,
};
use actix_multipart::form::{tempfile::TempFile, text::Text, MultipartForm};
use actix_web::{
    delete, get, post, put,
    web::{Data, Json, Path},
    HttpResponse, Responder,
};
use aws_sdk_s3::{types::ByteStream, Client};
use chrono::prelude::*;
use lazy_static::lazy_static;
use memmap::Mmap;
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as};
use std::{env, error::Error, fmt};
use utoipa::ToSchema;
use uuid::Uuid;

lazy_static! {
    static ref GAMES_BUCKET: String = env::var("S3_GAMES_BUCKET").unwrap();
}

#[derive(Debug, Clone)]
struct GameError {
    reason: String,
}

impl GameError {
    fn new(reason: &str) -> GameError {
        GameError {
            reason: reason.to_string(),
        }
    }
}
impl Error for GameError {}
impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "GameError: {}", self.reason)
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GameData {
    #[schema(example = "BrickBreaker")]
    name: String,
    #[schema(example = "Break bricks, get points")]
    description: String,
    #[schema(example = "ella")]
    author: String,
    tags: Vec<String>,
}

#[derive(Debug, MultipartForm)]
pub struct GameUpload {
    pub banner: TempFile,
    pub icon: TempFile,
    pub title: Text<String>,
    pub description: Text<String>,
    pub author: Text<String>,
    pub tags: Text<String>,
}

#[allow(dead_code)]
#[derive(ToSchema)]
pub struct GameUploadDoc {
    #[schema(format = Binary)]
    game: String,
    #[schema(format = Binary)]
    banner: String,
    #[schema(format = Binary)]
    icon: String,
    title: String,
    description: String,
    author: String,
    tags: Vec<String>,
}

#[derive(Debug, MultipartForm)]
pub struct FileUpload {
    file: TempFile,
}

#[allow(dead_code)]
#[derive(ToSchema)]
pub struct FileUploadDoc {
    #[schema(format = Binary)]
    file: String,
}

#[derive(Debug)]
enum ImageComponent {
    Banner,
    Icon,
}

impl ImageComponent {
    pub fn filename(&self) -> String {
        format!("{:?}", self).to_lowercase()
    }
}

#[utoipa::path(
    context_path = "/games",
    responses(
        (status = 200, description = "List all games", body = [GameWithTags]),
        (status = 500, description = "Error Created by Query"),
    )
)]
#[get("/")]
pub async fn get_all_games(state: Data<AppState>) -> impl Responder {
    match query_as::<_, GameWithTags>(
        "
        SELECT game.*,
            ROW(users.*)::users AS \"user\",
            array_remove(ARRAY_AGG(tags.*), NULL) AS \"tags\"
        FROM game
        LEFT JOIN game_tags ON game_tags.game_id = game.id
        LEFT JOIN tags ON tags.name = game_tags.tag_name
        LEFT JOIN users ON game.author = users.id
        GROUP BY game.id, users.id ORDER BY name ASC
        ",
    )
    .fetch_all(&state.db)
    .await
    {
        Ok(games) => HttpResponse::Ok().json(games),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

async fn verify_and_upload_game(
    game: TempFile,
    s3: &Client,
    uuid: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let game_content_type = game
        .content_type
        .as_ref()
        .map(|mime| mime.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());
    if game_content_type != "application/vnd.flatpak"
        && game_content_type != "application/octet-stream"
    {
        return Err(Box::new(GameError::new("Game provided is not a Flatpak!")));
    }
    // Afaik, this is only unsafe because outside processes (read: the OS) could
    // write to our file unsynchronized
    let file_memory_map = unsafe { Mmap::map(game.file.as_file()) }?;
    let flatpak = FlatpakFile::load(file_memory_map)?;
    let hash = flatpak.get_hash();
    let flatpak_ref: String = flatpak.get_metadata_key("ref")?;
    let components: Vec<&str> = flatpak_ref.split('/').collect();
    if components[0] != "app" {
        return Err(Box::new(GameError::new("Flatpak must be of type app")));
    }
    let app_id = format!("edu.rit.csh.devcade.game.id-{uuid}");
    if components[1] != app_id {
        return Err(Box::new(GameError::new(&format!(
            "Flatpak app id {} must be {app_id}",
            components[1]
        ))));
    }
    if components[2] != "x86_64" {
        return Err(Box::new(GameError::new(
            "Flatpak architecture must be x86_64",
        )));
    }
    if components[3] != "master" {
        return Err(Box::new(GameError::new("Flatpak branch must be master")));
    }

    let _ = s3
        .put_object()
        .key(format!("{}/{}.flatpak", uuid, uuid))
        .body(ByteStream::from_path(game.file.path()).await?)
        .bucket(&GAMES_BUCKET.to_string())
        .send()
        .await?;
    Ok(hash)
}

async fn verify_and_upload_image(
    image: TempFile,
    s3: &Client,
    image_type: ImageComponent,
    uuid: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let image_content_type = image
        .content_type
        .as_ref()
        .ok_or("Could not determine file type")?
        .clone();
    if image_content_type.type_() != "image" {
        return Err(Box::new(GameError::new(&format!(
            "{:?} provided is not an image",
            image_type
        ))));
    }
    let _ = s3
        .put_object()
        .key(format!(
            "{}/{}",
            uuid,
            image_type.filename(),
            //image_content_type.subtype()
        ))
        .body(ByteStream::from_path(image.file.path()).await?)
        .bucket(&GAMES_BUCKET.to_string())
        .send()
        .await?;
    Ok(())
}

async fn verify_and_upload(
    banner: TempFile,
    icon: TempFile,
    s3: &Client,
    uuid: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    verify_and_upload_image(banner, s3, ImageComponent::Banner, uuid).await?;
    verify_and_upload_image(icon, s3, ImageComponent::Icon, uuid).await?;
    Ok(())
}

#[utoipa::path(
    context_path = "/games",
    request_body(content=GameUploadDoc, content_type="multipart/form-data", description="Multipart Form. Contains flatpak file of game, banner, icon, name, description, and author"),
    responses(
        (status = 201, description = "Created new game"),
        (status = 400, description = "Invalid format of file upload"),
        (status = 401, description = "Invalid/Missing API Key"),
        (status = 500, description = "Error Created by Query"),
    ),
    security(
        ("api_key" = [])
    )
)]
#[post("/", wrap = "RequireApiKey")]
pub async fn add_game(
    state: Data<AppState>,
    MultipartForm(form): MultipartForm<GameUpload>,
) -> impl Responder {
    let uuid = Uuid::new_v4().to_string();
    match verify_and_upload(form.banner, form.icon, &state.s3, &uuid).await {
        Ok(()) => {
            let date = Local::now().date_naive();
            match query("INSERT INTO game VALUES ($1, $2, $3, $4, $5, $6)")
                .bind(&uuid)
                .bind(form.author.clone())
                .bind(date)
                .bind(form.title.clone())
                .bind(&None::<String>)
                .bind(form.description.clone())
                .execute(&state.db)
                .await
            {
                Ok(_) => {
                    for tag_name in form.tags.split(',') {
                        if tag_name.is_empty() {
                            continue;
                        }
                        if let Err(e) = query("INSERT INTO game_tags VALUES ($1, $2)")
                            .bind(&uuid)
                            .bind(tag_name)
                            .execute(&state.db)
                            .await
                        {
                            return HttpResponse::BadRequest().body(e.to_string());
                        }
                    }
                    HttpResponse::Created().json(Game {
                        id: uuid,
                        author: form.author.clone(),
                        upload_date: date,
                        name: form.title.clone(),
                        hash: None,
                        description: form.description.clone(),
                    })
                }
                Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
            }
        }
        Err(e) => HttpResponse::NotAcceptable().body(e.to_string()),
    }
}

#[utoipa::path(
    context_path = "/games",
    responses(
        (status = 200, description = "Get specified game", body = GameWithTags),
        (status = 400, description = "Missing game"),
        (status = 500, description = "Error Created by Query"),
    )
)]
#[get("/{id}")]
pub async fn get_game(state: Data<AppState>, path: Path<(String,)>) -> impl Responder {
    let (id,) = path.into_inner();
    match query_as::<_, GameWithTags>(
        "
        SELECT game.*,
            ROW(users.*)::users AS \"user\",
            array_remove(ARRAY_AGG(tags.*), NULL) AS \"tags\"
        FROM game
        LEFT JOIN game_tags ON game_tags.game_id = game.id
        LEFT JOIN tags ON tags.name = game_tags.tag_name
        LEFT JOIN users ON users.id = game.author
        WHERE game.id = $1
        GROUP BY game.id, users.id
        ",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    {
        Ok(game) => HttpResponse::Ok().json(game),
        Err(e) => HttpResponse::BadRequest().body(e.to_string()),
    }
}

#[utoipa::path(
    context_path = "/games",
    request_body(content=GameData, content_type="application/json", description="JSON with name, desc, and author"),
    responses(
        (status = 200, description = "Updated game"),
        (status = 400, description = "Missing game"),
        (status = 401, description = "Invalid/Missing API Key"),
        (status = 500, description = "Error Created by Query"),
    ),
    security(
        ("api_key" = [])
    )
)]
#[put("/{id}", wrap = "RequireApiKey")]
pub async fn edit_game(
    state: Data<AppState>,
    path: Path<(String,)>,
    game_data: Json<GameData>,
) -> impl Responder {
    let (id,) = path.into_inner();
    let mut transaction = state.db.begin().await.unwrap();
    match query_as::<_, Game>("SELECT * FROM game WHERE id = $1")
        .bind(&id)
        .fetch_one(&mut transaction)
        .await
    {
        Ok(game) => {
            match query("UPDATE game SET name = $1, description = $2 WHERE id = $3")
                .bind(game_data.name.clone())
                .bind(game_data.description.clone())
                .bind(&id)
                .execute(&mut transaction)
                .await
            {
                Ok(_) => {
                    if let Err(e) = query("DELETE FROM game_tags WHERE game_id =  $1")
                        .bind(&id)
                        .execute(&mut transaction)
                        .await
                    {
                        let _ = transaction.rollback().await;
                        return HttpResponse::InternalServerError().body(e.to_string());
                    };
                    for tag_name in game_data.tags.clone() {
                        if let Err(e) = query("INSERT INTO game_tags VALUES ($1, $2)")
                            .bind(&id)
                            .bind(tag_name)
                            .execute(&mut transaction)
                            .await
                        {
                            let _ = transaction.rollback().await;
                            return HttpResponse::BadRequest().body(e.to_string());
                        }
                    }
                    let _ = transaction.commit().await;
                    HttpResponse::Ok().json(Game {
                        id,
                        author: game.author,
                        upload_date: game.upload_date,
                        name: game_data.name.clone(),
                        hash: game.hash,
                        description: game_data.description.clone(),
                    })
                }
                Err(e) => {
                    let _ = transaction.rollback().await;
                    HttpResponse::InternalServerError().body(e.to_string())
                }
            }
        }
        Err(_) => {
            let _ = transaction.rollback().await;
            HttpResponse::BadRequest().body("Game ID Does Not Exist")
        }
    }
}

async fn delete_recursively(s3: &Client, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    s3.delete_object()
        .bucket(&GAMES_BUCKET.to_string())
        .key(format!("{}/{}.flatpak", id, id))
        .send()
        .await?;
    s3.delete_object()
        .bucket(&GAMES_BUCKET.to_string())
        .key(format!("{}/icon", id))
        .send()
        .await?;
    s3.delete_object()
        .bucket(&GAMES_BUCKET.to_string())
        .key(format!("{}/banner", id))
        .send()
        .await?;
    Ok(())
}

#[utoipa::path(
    context_path = "/games",
    responses(
        (status = 200, description = "Delete game"),
        (status = 400, description = "Missing game"),
        (status = 401, description = "Invalid/Missing API Key"),
        (status = 500, description = "Error Created by Deletion"),
    ),
    params(
        ("id", description = "Unique id of game")
    ),
    security(
        ("api_key" = [])
    )
)]
#[delete("/{id}", wrap = "RequireApiKey")]
pub async fn delete_game(state: Data<AppState>, path: Path<(String,)>) -> impl Responder {
    let (id,) = path.into_inner();
    if query_as::<_, Game>("SELECT * FROM game WHERE id = $1")
        .bind(&id)
        .fetch_one(&state.db)
        .await
        .is_err()
    {
        return HttpResponse::BadRequest().body("Game ID Does Not Exist");
    }
    match delete_recursively(&state.s3, &id).await {
        Ok(_) => {
            match query("DELETE FROM game WHERE id = $1")
                .bind(&id)
                .execute(&state.db)
                .await
            {
                Ok(_) => match query("DELETE FROM game_tags WHERE game_id = $1")
                    .bind(&id)
                    .execute(&state.db)
                    .await
                {
                    Ok(_) => HttpResponse::Ok().finish(),
                    Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
                },
                Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
            }
        }
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[utoipa::path(
    context_path = "/games",
    responses(
        (status = 200, description = "Provide game source flatpak", content_type="application/vnd.flatpak"),
        (status = 400, description = "Missing game"),
        (status = 500, description = "Error Created by Query"),
    ),
    params(
        ("id", description = "Unique id of game")
    ),
)]
#[get("/{id}/game")]
pub async fn get_binary(state: Data<AppState>, path: Path<(String,)>) -> impl Responder {
    let (id,) = path.into_inner();
    if query_as::<_, Game>("SELECT * FROM game WHERE id = $1")
        .bind(&id)
        .fetch_one(&state.db)
        .await
        .is_err()
    {
        return HttpResponse::BadRequest().body("Game ID Does Not Exist");
    }
    match state
        .s3
        .get_object()
        .bucket(&GAMES_BUCKET.to_string())
        .key(format!("{}/{}.flatpak", id, id))
        .send()
        .await
    {
        Ok(objout) => HttpResponse::Ok().streaming(objout.body),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error getting object: {}", e)),
    }
}

#[utoipa::path(
    context_path = "/games",
    request_body(content=FileUploadDoc, content_type="multipart/form-data", description="Flatpak of game publish folder"),
    responses(
        (status = 200, description = "Updated Game Binary"),
        (status = 400, description = "Missing game"),
        (status = 401, description = "Invalid/Missing API Key"),
        (status = 500, description = "Error Created by Query"),
    ),
    params(
        ("id", description = "Unique id of game")
    ),
    security(
        ("api_key" = [])
    )
)]
#[put("/{id}/game", wrap = "RequireApiKey")]
pub async fn update_binary(
    state: Data<AppState>,
    path: Path<(String,)>,
    MultipartForm(form): MultipartForm<FileUpload>,
) -> impl Responder {
    let (id,) = path.into_inner();
    match query_as::<_, Game>("SELECT * FROM game WHERE id = $1")
        .bind(&id)
        .fetch_one(&state.db)
        .await
    {
        Ok(game) => match verify_and_upload_game(form.file, &state.s3, &id).await {
            Ok(hash) => {
                match query("UPDATE game SET hash = $1 WHERE id = $2")
                    .bind(&hash)
                    .bind(&id)
                    .execute(&state.db)
                    .await
                {
                    Ok(_) => HttpResponse::Ok().json(Game {
                        id,
                        author: game.author,
                        upload_date: game.upload_date,
                        name: game.name,
                        hash: Some(hash),
                        description: game.description,
                    }),
                    Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
                }
            }
            Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
        },
        Err(_) => HttpResponse::BadRequest().body("Game ID Does Not Exist"),
    }
}

#[utoipa::path(
    context_path = "/games",
    responses(
        (status = 200, description = "Provide game banner"),
        (status = 400, description = "Missing game"),
        (status = 500, description = "Error Created by Query"),
    ),
    params(
        ("id", description = "Unique id of game")
    ),
)]
#[get("/{id}/banner")]
pub async fn get_banner(state: Data<AppState>, path: Path<(String,)>) -> impl Responder {
    let (id,) = path.into_inner();
    if query_as::<_, Game>("SELECT * FROM game WHERE id = $1")
        .bind(&id)
        .fetch_one(&state.db)
        .await
        .is_err()
    {
        return HttpResponse::BadRequest().body("Game ID Does Not Exist");
    }
    match state
        .s3
        .get_object()
        .bucket(&GAMES_BUCKET.to_string())
        .key(format!("{}/banner", id))
        .send()
        .await
    {
        Ok(objout) => {
            let bytestream = objout.body.collect().await;
            match bytestream {
                Ok(bytes) => HttpResponse::Ok().body(bytes.into_bytes()),
                Err(e) => HttpResponse::InternalServerError()
                    .body(format!("Error getting object body: {}", e)),
            }
        }
        Err(e) => HttpResponse::InternalServerError().body(format!("Error getting object: {}", e)),
    }
}

#[utoipa::path(
    context_path = "/games",
    request_body(content=FileUploadDoc, content_type="multipart/form-data", description="Game banner"),
    responses(
        (status = 200, description = "Updated Game Banner"),
        (status = 400, description = "Missing game"),
        (status = 401, description = "Invalid/Missing API Key"),
        (status = 500, description = "Error Created by Query"),
    ),
    params(
        ("id", description = "Unique id of game")
    ),
    security(
        ("api_key" = [])
    )
)]
#[put("/{id}/banner", wrap = "RequireApiKey")]
pub async fn update_banner(
    state: Data<AppState>,
    path: Path<(String,)>,
    MultipartForm(form): MultipartForm<FileUpload>,
) -> impl Responder {
    let (id,) = path.into_inner();
    match query_as::<_, Game>("SELECT * FROM game WHERE id = $1")
        .bind(&id)
        .fetch_one(&state.db)
        .await
    {
        Ok(_) => {
            match verify_and_upload_image(form.file, &state.s3, ImageComponent::Banner, &id).await {
                Ok(_) => HttpResponse::Ok().finish(),
                Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
            }
        }
        Err(_) => HttpResponse::BadRequest().body("Game ID Does Not Exist"),
    }
}

#[utoipa::path(
    context_path = "/games",
    responses(
        (status = 200, description = "Provide game icon"),
        (status = 400, description = "Missing game"),
        (status = 500, description = "Error Created by Query"),
    ),
    params(
        ("id", description = "Unique id of game")
    ),
)]
#[get("/{id}/icon")]
pub async fn get_icon(state: Data<AppState>, path: Path<(String,)>) -> impl Responder {
    let (id,) = path.into_inner();
    if query_as::<_, Game>("SELECT * FROM game WHERE id = $1")
        .bind(&id)
        .fetch_one(&state.db)
        .await
        .is_err()
    {
        return HttpResponse::BadRequest().body("Game ID Does Not Exist");
    }
    match state
        .s3
        .get_object()
        .bucket(&GAMES_BUCKET.to_string())
        .key(format!("{}/icon", id))
        .send()
        .await
    {
        Ok(objout) => {
            let bytestream = objout.body.collect().await;
            match bytestream {
                Ok(bytes) => HttpResponse::Ok().body(bytes.into_bytes()),
                Err(e) => HttpResponse::InternalServerError()
                    .body(format!("Error getting object body: {}", e)),
            }
        }
        Err(e) => HttpResponse::InternalServerError().body(format!("Error getting object: {}", e)),
    }
}

#[utoipa::path(
    context_path = "/games",
    request_body(content=FileUploadDoc, content_type="multipart/form-data", description="Game Icon"),
    responses(
        (status = 200, description = "Updated Game Icon"),
        (status = 400, description = "Missing game"),
        (status = 401, description = "Invalid/Missing API Key"),
        (status = 500, description = "Error Created by Query"),
    ),
    params(
        ("id", description = "Unique id of game")
    ),
    security(
        ("api_key" = [])
    )
)]
#[put("/{id}/icon", wrap = "RequireApiKey")]
pub async fn update_icon(
    state: Data<AppState>,
    path: Path<(String,)>,
    MultipartForm(form): MultipartForm<FileUpload>,
) -> impl Responder {
    let (id,) = path.into_inner();
    match query_as::<_, Game>("SELECT * FROM game WHERE id = $1")
        .bind(&id)
        .fetch_one(&state.db)
        .await
    {
        Ok(_) => {
            match verify_and_upload_image(form.file, &state.s3, ImageComponent::Icon, &id).await {
                Ok(_) => HttpResponse::Ok().finish(),
                Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
            }
        }
        Err(_) => HttpResponse::BadRequest().body("Game ID Does Not Exist"),
    }
}
