use aws_sdk_s3::Client;
use serde::{Deserialize, Serialize};
use sqlx::{
    postgres::{PgHasArrayType, PgTypeInfo},
    types::chrono::NaiveDate,
    FromRow, Pool, Postgres,
};
use utoipa::{self, ToSchema};

#[derive(Serialize, Deserialize, FromRow, ToSchema, Clone, PartialEq)]
pub struct Game {
    #[schema(example = "a1c6cef6-d987-4225-8bc4-def387e8b5bf")]
    pub id: String,
    #[schema(example = "ella")]
    pub author: String,
    #[schema(example = "2023-03-20")]
    pub upload_date: NaiveDate,
    #[schema(example = "BrickBreaker")]
    pub name: String,
    #[schema(example = "6485ae710b078ffe0b97083b4cd1719920ed0d5ae5e77d1fbf6a7e3b88c35c78")]
    pub hash: Option<String>,
    #[schema(example = "Break bricks, get points")]
    pub description: String,
}

#[derive(Serialize, Deserialize, FromRow, ToSchema, Clone, PartialEq, Debug)]
pub struct GameWithTags {
    #[schema(example = "a1c6cef6-d987-4225-8bc4-def387e8b5bf")]
    pub id: String,
    #[schema(example = "103956139596074306830")]
    pub author: String,
    #[schema(example = "2023-03-20")]
    pub upload_date: NaiveDate,
    #[schema(example = "SardineCanOpeningSimulatorTheGame")]
    pub name: String,
    #[schema(example = "kisQdebh0jnh6rb+bqQeM1EAxrg=")]
    pub hash: Option<String>,
    #[schema(example = "Huh")]
    pub description: String,
    #[schema(
        example = "[{\"name\": \"authrequired\", \"description\": \"Required CSH Authentication to Access\"}]"
    )]
    pub tags: Vec<Tag>,
    #[schema(example = "{
            \"id\": \"103956139596074306830\",
            \"user_type\": \"GOOGLE\",
            \"first_name\": \"Wilson\",
            \"last_name\": \"Mcdade\",
            \"picture\": \"https://lh3.googleusercontent.com/a/AGNmyxYS7ZmwC4Uw2ZhBlOdMvpIU7z3FwfRRkvw66d3r=s96-c\",
            \"admin\": false,
            \"email\": \"wam2134@g.rit.edu\"
        }")]
    pub user: User,
}

impl GameWithTags {
    pub fn new(game: Game, tags: Vec<Tag>, user: User) -> GameWithTags {
        GameWithTags {
            id: game.id.clone(),
            author: game.author.clone(),
            upload_date: game.upload_date,
            name: game.name.clone(),
            hash: game.hash.clone(),
            description: game.description,
            tags,
            user,
        }
    }
}

#[derive(sqlx::Type, Serialize, Deserialize, FromRow, ToSchema, Clone, PartialEq, Debug)]
pub struct Tag {
    #[schema(example = "authrequired")]
    pub name: String,
    #[schema(example = "Required CSH Authentication to Access")]
    pub description: String,
}

impl PgHasArrayType for Tag {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("_tags")
    }
}

#[derive(sqlx::Type, Serialize, Deserialize, Clone, ToSchema, Debug, PartialEq)]
pub enum UserType {
    CSH,
    GOOGLE,
}

#[derive(sqlx::Type, Serialize, Deserialize, FromRow, ToSchema, Clone, PartialEq, Debug)]
#[sqlx(type_name = "users")]
pub struct User {
    #[schema(example = "skyz")]
    pub id: String,
    #[schema(example = UserType::CSH)]
    pub user_type: UserType,
    #[schema(example = "Joe")]
    pub first_name: String,
    #[schema(example = "Abbate")]
    pub last_name: String,
    #[schema(example = "IMAGE_URL")]
    pub picture: String,
    #[schema(example = true)]
    pub admin: bool,
    #[schema(example = "skyz@csh.rit.edu")]
    pub email: String,
}

impl User {
    pub fn from_csh(username: &str, first_name: &str, last_name: &str, admin: bool) -> User {
        User {
            id: username.to_string(),
            user_type: UserType::CSH,
            first_name: first_name.to_string(),
            last_name: last_name.to_string(),
            picture: format!("https://profiles.csh.rit.edu/image/{}", username),
            email: format!("{}@csh.rit.edu", username),
            admin,
        }
    }
}

pub struct AppState {
    pub db: Pool<Postgres>,
    pub s3: Client,
}
