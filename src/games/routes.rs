use crate::{
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
use bytes::Bytes;
use chrono::prelude::*;
use image::{imageops, load_from_memory, ImageBuffer};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use sqlx::{query, query_as, Postgres, Row};
use std::{
    env,
    error::Error,
    fmt,
    fs::File,
    io::{BufReader, Cursor, Read},
};
use utoipa::ToSchema;
use uuid::Uuid;
use zip::read::ZipArchive;

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
    pub game: TempFile,
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

#[utoipa::path(
    context_path = "/games",
    responses(
        (status = 200, description = "Get a hash of all games", body = [GameWithTags]),
        (status = 500, description = "Error Created by Query"),
    )
)]
#[get("/hash")]
pub async fn get_list_hash(state: Data<AppState>) -> impl Responder {
    println!("chom");
    let hashes: Vec<String> = match query::<Postgres>("select hash from game order by name asc")
        .fetch_all(&state.db)
        .await
    {
        Ok(hashes) => hashes
            .iter()
            .map(|x| x.get::<String, &str>("hash"))
            .collect(),
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    println!("chom");

    let hash_string = hashes.join("");

    let mut hasher = Sha1::new();
    hasher.update(&hash_string);
    let hexes = hasher.finalize();
    let mut out = String::new();
    for hex in hexes {
        out.push_str(&format!("{:02x?}", hex));
    }
    println!("chom");

    HttpResponse::Ok().json(hash_string)
}

async fn verify_and_upload_game(
    game: TempFile,
    s3: &Client,
    uuid: Option<String>,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let game_content_type = game
        .content_type
        .as_ref()
        .ok_or("Could not determine file type")?
        .clone();
    if game_content_type != "application/zip" {
        return Err(Box::new(GameError::new("Game provided is not a Zip")));
    }
    let uuid = uuid.unwrap_or(Uuid::new_v4().to_string());
    {
        let zip_file = File::open(game.file.path())?;
        let mut zip_archive = ZipArchive::new(zip_file)?;
        let publish = zip_archive
            .by_name("publish/")
            .map_err(|_| GameError::new("publish directory not found"))?;
        if !publish.is_dir() {
            return Err(Box::new(GameError::new("publish is not a directory")));
        }
    }
    let hash = sha1sum(game.file.path().display().to_string())?;
    let _ = s3
        .put_object()
        .key(format!("{}/{}.zip", uuid, uuid))
        .body(ByteStream::from_path(game.file.path()).await?)
        .bucket(&GAMES_BUCKET.to_string())
        .send()
        .await?;
    Ok((uuid, hash))
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
    let image_buffer = Vec::new();
    let mut cursor = Cursor::new(image_buffer);
    imageops::resize(
        &load_from_memory(
            &ByteStream::from_path(image.file.path())
                .await?
                .collect()
                .await?
                .into_bytes(),
        )?,
        match image_type {
            ImageComponent::Icon => 150,
            ImageComponent::Banner => 850,
        },
        match image_type {
            ImageComponent::Icon => 150,
            ImageComponent::Banner => 400,
        },
        imageops::FilterType::Lanczos3,
    )
    .write_to(&mut cursor, image::ImageFormat::Png)?;
    let _ = s3
        .put_object()
        .key(format!(
            "{}/{}",
            uuid,
            image_type.filename(),
            //image_content_type.subtype()
        ))
        .body(ByteStream::from(Bytes::copy_from_slice(
            ImageBuffer::into_raw(imageops::resize(
                &load_from_memory(
                    &ByteStream::from_path(image.file.path())
                        .await?
                        .collect()
                        .await?
                        .into_bytes(),
                )?,
                match image_type {
                    ImageComponent::Banner => 850,
                    ImageComponent::Icon => 150,
                },
                match image_type {
                    ImageComponent::Banner => 400,
                    ImageComponent::Icon => 150,
                },
                imageops::FilterType::Lanczos3,
            ))
            .as_slice(),
        )))
        .bucket(&GAMES_BUCKET.to_string())
        .send()
        .await?;
    Ok(())
}

async fn verify_and_upload(
    game: TempFile,
    banner: TempFile,
    icon: TempFile,
    s3: &Client,
    uuid: Option<String>,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let (uuid, hash) = verify_and_upload_game(game, s3, uuid).await?;
    verify_and_upload_image(banner, s3, ImageComponent::Banner, &uuid).await?;
    verify_and_upload_image(icon, s3, ImageComponent::Icon, &uuid).await?;
    Ok((uuid, hash))
}

#[utoipa::path(
    context_path = "/games",
    request_body(content=GameUploadDoc, content_type="multipart/form-data", description="Multipart Form. Contains zip file of game, banner, icon, name, description, and author"),
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
    match verify_and_upload(form.game, form.banner, form.icon, &state.s3, None).await {
        Ok((uuid, hash)) => {
            let date = Local::now().date_naive();
            match query("INSERT INTO game VALUES ($1, $2, $3, $4, $5, $6)")
                .bind(&uuid)
                .bind(form.author.clone())
                .bind(date)
                .bind(form.title.clone())
                .bind(&hash)
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
                        hash,
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
        .key(format!("{}/{}.zip", id, id))
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
        (status = 200, description = "Provide game source zip", content_type="application/zip"),
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
        .key(format!("{}/{}.zip", id, id))
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
    request_body(content=FileUploadDoc, content_type="multipart/form-data", description="Zip of game publish folder"),
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
        Ok(game) => match verify_and_upload_game(form.file, &state.s3, Some(id.clone())).await {
            Ok((_, hash)) => HttpResponse::Ok().json(Game {
                id,
                author: game.author,
                upload_date: game.upload_date,
                name: game.name,
                hash,
                description: game.description,
            }),
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

pub fn sha1sum(filepath: String) -> Result<String, Box<dyn std::error::Error>> {
    let f = File::open(&filepath)?;
    let mut reader = BufReader::new(f);
    let mut buffer = Vec::new();

    // Read file into vector.
    reader.read_to_end(&mut buffer)?;

    let mut hasher = Sha1::new();
    hasher.update(&buffer);
    let hexes = hasher.finalize();
    let mut out = String::new();
    for hex in hexes {
        out.push_str(&format!("{:02x?}", hex));
    }
    Ok(out)
}
