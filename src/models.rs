use crate::schema::*;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::PgConnection;
use jsonwebtoken::*;
use rocket::{
	http::Status,
	request::{FromRequest, Outcome},
	serde::{Deserialize, Serialize},
};

pub type ConnectionPool = Pool<ConnectionManager<PgConnection>>;
pub type ConnectionState = rocket::State<ConnectionPool>;

pub fn get_connection(
	connection: &ConnectionState,
) -> PooledConnection<ConnectionManager<PgConnection>> {
	connection.get().unwrap()
}

pub struct Config {
	pub decoding_key: DecodingKey,
	pub encoding_key: EncodingKey,
	pub discord_id: String,
	pub discord_secret: String,
	pub base_url: String,
	pub max_image_size: u64,
	pub max_file_size: u64,
	pub cloudflare_image_token: String,
	pub cloudflare_account_id: String,
	pub admins: Vec<i64>,
	pub tag_toml: TagToml,
	pub theme_toml: ThemeToml,
	pub webui_limit: i64,
	pub gtag: String,
	pub game_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Theme {
	pub id: i32,
	pub url: String,
	pub name: String,
}

impl Default for Theme {
	fn default() -> Self {
		Self {
			id: 0,
			url: String::from(
				"https://cdnjs.cloudflare.com/ajax/libs/bootswatch/5.2.0/darkly/bootstrap.min.css",
			),
			name: String::from("Darkly"),
		}
	}
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThemeToml {
	pub themes: Vec<Theme>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TagToml {
	pub game_tags: Vec<Tag>,
	pub type_tags: Vec<Tag>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tag {
	pub id: i32,
	pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct Token {
	pub iat: i64,
	pub exp: i64,
	pub user_id: i64,
}

pub fn create_jwt(user_id: i64, config: &rocket::State<Config>) -> String {
	let time = chrono::offset::Utc::now().timestamp();
	let token_data = Token {
		iat: time,
		exp: time + 30 * 24 * 60 * 60,
		user_id,
	};
	encode(&Header::default(), &token_data, &config.encoding_key).unwrap_or_default()
}

#[derive(Debug)]
pub enum GenericError {
	Missing,
	Invalid,
}

pub struct Verified {}

impl Verified {
	pub fn verify(token: &str, config: &Config) -> Outcome<Self, GenericError> {
		let token = decode::<Token>(token, &config.decoding_key, &Validation::default());
		if token.is_err() {
			Outcome::Failure((Status::Unauthorized, GenericError::Invalid))
		} else {
			Outcome::Success(Self {})
		}
	}
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Verified {
	type Error = GenericError;
	async fn from_request(request: &'r rocket::Request<'_>) -> Outcome<Self, Self::Error> {
		let config = request.rocket().state::<Config>().unwrap();
		match request.headers().get_one("Authorization") {
			None => match request.cookies().get_pending("jwt") {
				None => Outcome::Failure((Status::Unauthorized, GenericError::Missing)),
				Some(cookie) => {
					let token = cookie.value();
					Self::verify(token, config)
				}
			},
			Some(token) => {
				let token = token.replace("Bearer ", "");
				Self::verify(&token, config)
			}
		}
	}
}

impl User {
	pub fn verify(
		token: &str,
		connection: &ConnectionPool,
		config: &Config,
	) -> Outcome<Self, GenericError> {
		let token_data = decode::<Token>(token, &config.decoding_key, &Validation::default());
		let token_data = match token_data {
			Ok(token_data) => token_data,
			Err(_) => return Outcome::Failure((Status::Unauthorized, GenericError::Invalid)),
		};
		let result =
			crate::database::get_user(&mut connection.get().unwrap(), token_data.claims.user_id);
		match result {
			Some(user) => Outcome::Success(user),
			None => Outcome::Failure((Status::BadRequest, GenericError::Invalid)),
		}
	}
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for User {
	type Error = GenericError;
	async fn from_request(request: &'r rocket::Request<'_>) -> Outcome<Self, Self::Error> {
		let config = request.rocket().state::<Config>().unwrap();
		match request.headers().get_one("Authorization") {
			None => match request.cookies().get_pending("jwt") {
				None => Outcome::Failure((Status::Unauthorized, GenericError::Missing)),
				Some(cookie) => {
					let token = cookie.value();
					let connection = request.rocket().state::<ConnectionPool>().unwrap();
					Self::verify(token, connection, config)
				}
			},
			Some(token) => {
				let token = token.replace("Bearer ", "");
				let connection = request.rocket().state::<ConnectionPool>().unwrap();
				Self::verify(&token, connection, config)
			}
		}
	}
}

#[derive(Debug)]
pub struct HttpIp {
	pub ip: std::net::IpAddr,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for HttpIp {
	type Error = GenericError;
	async fn from_request(request: &'r rocket::Request<'_>) -> Outcome<Self, Self::Error> {
		match request.headers().get_one("x-forwarded-for") {
			None => Outcome::Failure((Status::Unauthorized, GenericError::Missing)),
			Some(header) => Outcome::Success(Self {
				ip: header.parse().unwrap(),
			}),
		}
	}
}

#[derive(Queryable, Serialize, Deserialize, Default)]
pub struct UserStats {
	pub likes: i64,
	pub dislikes: i64,
	pub downloads: i64,
}

#[derive(Serialize, Deserialize)]
pub struct PostUnidentified {
	pub name: String,
	pub text: String,
	pub text_short: String,
	pub image: Option<String>,
	pub images_extra: Option<Vec<String>>,
	pub link: String,
	pub game_tag: i32,
	pub type_tag: i32,
	pub change: Option<String>,
	pub change_download: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct PostMetadata {
	pub name: String,
	pub text: String,
	pub text_short: String,
	pub game_tag: i32,
	pub type_tag: i32,
	pub change: Option<String>,
}

#[derive(Queryable, Serialize, Deserialize, Default)]
pub struct ShortPost {
	pub id: i32,
	pub name: String,
	pub text_short: String,
	pub image: String,
	pub game_tag: i32,
	pub type_tag: i32,
	pub likes: i64,
	pub dislikes: i64,
	pub downloads: i64,
}

#[derive(Queryable, Serialize, Deserialize, Default)]
pub struct ShortPostNoLikes {
	pub id: i32,
	pub name: String,
	pub text_short: String,
	pub image: String,
	pub game_tag: i32,
	pub type_tag: i32,
	pub downloads: i64,
}

#[derive(Queryable, Serialize, Deserialize)]
pub struct DetailedPostNoDepends {
	pub id: i32,
	pub name: String,
	pub text: String,
	pub text_short: String,
	pub image: String,
	pub images_extra: Vec<String>,
	pub link: String,
	pub date: chrono::NaiveDateTime,
	pub game_tag: i32,
	pub type_tag: i32,
	pub downloads: i64,
	pub user: User,
}

#[derive(Serialize, Deserialize)]
pub struct DetailedPost {
	pub id: i32,
	pub name: String,
	pub text: String,
	pub text_short: String,
	pub dependencies: Vec<DetailedPostNoDepends>,
	pub image: String,
	pub images_extra: Vec<String>,
	pub link: String,
	pub date: chrono::NaiveDateTime,
	pub game_tag: i32,
	pub type_tag: i32,
	pub likes: i64,
	pub dislikes: i64,
	pub downloads: i64,
	pub user: User,
	pub changelogs: Vec<Changelog>,
	pub comments: Vec<Comment>,
}

#[derive(Queryable, Serialize, Deserialize, Default)]
pub struct ShortUserPosts {
	pub post: ShortPost,
	pub user: User,
}

#[derive(Queryable, Serialize, Deserialize, Default)]
pub struct ShortUserPostsNoLikes {
	pub post: ShortPostNoLikes,
	pub user: User,
}

#[derive(Queryable, Serialize, Deserialize, Default, Clone)]
pub struct User {
	pub id: i64,
	pub name: String,
	pub avatar: String,
}

#[derive(Insertable)]
#[diesel(table_name = users)]
pub struct NewUser<'a> {
	pub user_id: i64,
	pub user_name: &'a str,
	pub user_avatar: &'a str,
}

#[derive(Queryable, Serialize, Deserialize)]
pub struct Post {
	pub id: i32,
	pub name: String,
	pub text: String,
	pub text_short: String,
	pub image: String,
	pub images_extra: Vec<String>,
	pub uploader: i64,
	pub link: String,
	pub date: chrono::NaiveDateTime,
	pub game_tag: i32,
	pub type_tag: i32,
	pub downloads: i64,
}

#[derive(Insertable)]
#[diesel(table_name = posts)]
pub struct NewPost<'a> {
	pub post_name: &'a str,
	pub post_text: &'a str,
	pub post_text_short: &'a str,
	pub post_image: &'a str,
	pub post_images_extra: &'a Vec<String>,
	pub post_uploader: i64,
	pub post_link: &'a str,
	pub post_game_tag: i32,
	pub post_type_tag: i32,
	pub post_downloads: i64,
}

#[derive(Queryable, Serialize, Deserialize)]
pub struct LikedPost {
	pub id: i32,
	pub post: i32,
	pub user: i64,
}

#[derive(Insertable)]
#[diesel(table_name = users_liked_posts)]
pub struct NewLikedPost {
	pub post_id: i32,
	pub user_id: i64,
}

#[derive(Queryable, Serialize, Deserialize)]
pub struct DislikedPost {
	pub id: i32,
	pub post: i32,
	pub user: i64,
}

#[derive(Insertable)]
#[diesel(table_name = users_disliked_posts)]
pub struct NewDislikedPost {
	pub post_id: i32,
	pub user_id: i64,
}

#[derive(Queryable, Serialize, Deserialize)]
pub struct Report {
	pub id: i32,
	pub user: User,
	pub post: ShortPost,
	pub description: String,
}

#[derive(Queryable, Serialize, Deserialize)]
pub struct Changelog {
	pub description: String,
	pub time: chrono::NaiveDateTime,
	pub download: Option<String>,
}

#[derive(Queryable, Serialize, Deserialize)]
pub struct Comment {
	pub id: i32,
	pub user: User,
	pub text: String,
	pub parent: Option<i32>,
	pub date: chrono::NaiveDateTime,
}

#[derive(Queryable, Serialize, Deserialize)]
pub struct SitemapInfo {
	pub id: i32,
	pub date: chrono::NaiveDateTime,
}

#[derive(Queryable, Serialize, Deserialize)]
pub struct PostUpdateTime {
	pub id: i32,
	pub date: chrono::NaiveDateTime,
}
