use super::schema::*;
use dotenv::dotenv;
use jsonwebtoken::*;
use lazy_static::lazy_static;
use rocket::{
	http::Status,
	request::{FromRequest, Outcome},
	serde::{Deserialize, Serialize},
};
use std::{env, io::Read};

lazy_static! {
	pub static ref DECODE_KEY: DecodingKey = {
		dotenv().ok();
		let secret = env::var("SECRET_KEY").expect("SECRET_KEY must exist");
		DecodingKey::from_secret(secret.as_bytes())
	};
	pub static ref ENCODE_KEY: EncodingKey = {
		dotenv().ok();
		let secret = env::var("SECRET_KEY").expect("SECRET_KEY must exist");
		EncodingKey::from_secret(secret.as_bytes())
	};
	pub static ref DISCORD_ID: String = {
		dotenv().ok();
		env::var("DISCORD_ID").expect("DISCORD_ID must exist")
	};
	pub static ref DISCORD_SECRET: String = {
		dotenv().ok();
		env::var("DISCORD_SECRET").expect("DISCORD_SECRET must exist")
	};
	pub static ref BASE_URL: String = {
		dotenv().ok();
		env::var("BASE_URL").expect("BASE_URL must exist")
	};
	pub static ref MAX_IMAGE_SIZE: u64 = {
		dotenv().ok();
		let size = env::var("MAX_IMAGE_SIZE").expect("MAX_IMAGE_SIZE must exist");
		size.parse::<u64>().unwrap()
	};
	pub static ref MAX_FILE_SIZE: u64 = {
		dotenv().ok();
		let size = env::var("MAX_FILE_SIZE").expect("MAX_FILE_SIZE must exist");
		size.parse::<u64>().unwrap()
	};
	pub static ref CLOUDFLARE_IMAGE_TOKEN: String = {
		dotenv().ok();
		env::var("CLOUDFLARE_IMAGE_TOKEN").expect("CLOUDFLARE_IMAGE_TOKEN must exist")
	};
	pub static ref CLOUDFLARE_ACCOUNT_ID: String = {
		dotenv().ok();
		env::var("CLOUDFLARE_ACCOUNT_ID").expect("CLOUDFLARE_ACCOUNT_ID must exist")
	};
	pub static ref TAG_TOML: TagToml = {
		let mut tag_file =
			std::fs::File::open("static/tags.toml").expect("static/tags.toml must exist");
		let mut tag_toml = String::new();
		tag_file
			.read_to_string(&mut tag_toml)
			.expect("static/tags.toml must be a valid toml file");
		toml::from_str(&tag_toml).expect("static/tags.toml must be a valid tags toml file")
	};
	pub static ref ADMINS: Vec<i64> = {
		dotenv().ok();
		let admin_str = env::var("ADMIN_IDS").expect("ADMIN_IDS must exist");
		admin_str
			.split(',')
			.map(|x| x.parse::<i64>().unwrap())
			.collect()
	};
	pub static ref THEMES_TOML: ThemeToml = {
		let mut theme_file =
			std::fs::File::open("static/themes.toml").expect("static/themes.toml must exist");
		let mut theme_toml = String::new();
		theme_file
			.read_to_string(&mut theme_toml)
			.expect("static/themes.toml must be a valid toml file");
		toml::from_str(&theme_toml).expect("static/themes.toml must be a valid themes toml file")
	};
	pub static ref WEBUI_LIMIT: i64 = {
		dotenv().ok();
		let limit = env::var("WEBUI_LIMIT").expect("WEBUI_LIMIT must exist");
		limit.parse::<i64>().unwrap()
	};
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Theme {
	pub id: i32,
	pub url: String,
	pub name: String,
}

impl Default for Theme {
	fn default() -> Self {
		Theme {
			id: 0,
			url: String::from(
				"https://cdnjs.cloudflare.com/ajax/libs/bootswatch/5.2.0/darkly/bootstrap.min.css",
			),
			name: String::from("Darkly"),
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThemeToml {
	pub themes: Vec<Theme>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TagToml {
	pub game_tags: Vec<Tag>,
	pub type_tags: Vec<Tag>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tag {
	pub id: i32,
	pub name: String,
}

pub type ConnectionState = rocket::State<std::sync::Mutex<diesel::PgConnection>>;

#[derive(Serialize, Deserialize)]
pub struct Token {
	pub iat: i64,
	pub exp: i64,
	pub user_id: i64,
}

#[must_use]
pub fn create_jwt(user_id: i64) -> String {
	let time = chrono::offset::Utc::now().timestamp();
	let token_data = Token {
		iat: time,
		exp: time + 604_800,
		user_id,
	};
	encode(&Header::default(), &token_data, &ENCODE_KEY).unwrap_or_default()
}

#[derive(Debug)]
pub enum GenericErorr {
	Missing,
	Invalid,
}

pub struct Verified {}

impl Verified {
	pub fn verify(token: &str) -> Outcome<Self, GenericErorr> {
		let token = decode::<Token>(token, &DECODE_KEY, &Validation::default());
		if token.is_err() {
			Outcome::Failure((Status::Unauthorized, GenericErorr::Invalid))
		} else {
			Outcome::Success(Self {})
		}
	}
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Verified {
	type Error = GenericErorr;
	async fn from_request(request: &'r rocket::Request<'_>) -> Outcome<Self, Self::Error> {
		match request.headers().get_one("Authorization") {
			None => match request.cookies().get_pending("jwt") {
				None => Outcome::Failure((Status::Unauthorized, GenericErorr::Missing)),
				Some(cookie) => {
					let token = cookie.value();
					Self::verify(token)
				}
			},
			Some(token) => {
				let token = token.replace("Bearer ", "");
				Self::verify(&token)
			}
		}
	}
}

impl User {
	pub fn verify(
		token: &str,
		connection: &std::sync::Mutex<diesel::PgConnection>,
	) -> Outcome<Self, GenericErorr> {
		let token_data = decode::<Token>(token, &DECODE_KEY, &Validation::default());
		if let Ok(token_data) = token_data {
			let result =
				crate::users::get_user(&mut connection.lock().unwrap(), token_data.claims.user_id);
			match result {
				Ok(user) => Outcome::Success(user),
				Err(status) => Outcome::Failure((status, GenericErorr::Invalid)),
			}
		} else {
			Outcome::Failure((Status::Unauthorized, GenericErorr::Invalid))
		}
	}
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for User {
	type Error = GenericErorr;
	async fn from_request(request: &'r rocket::Request<'_>) -> Outcome<Self, Self::Error> {
		match request.headers().get_one("Authorization") {
			None => match request.cookies().get_pending("jwt") {
				None => Outcome::Failure((Status::Unauthorized, GenericErorr::Missing)),
				Some(cookie) => {
					let token = cookie.value();
					let connection = request
						.rocket()
						.state::<std::sync::Mutex<diesel::PgConnection>>()
						.unwrap();
					Self::verify(token, connection)
				}
			},
			Some(token) => {
				let token = token.replace("Bearer ", "");
				let connection = request
					.rocket()
					.state::<std::sync::Mutex<diesel::PgConnection>>()
					.unwrap();
				Self::verify(&token, connection)
			}
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
	pub image: String,
	pub images_extra: Vec<String>,
	pub link: String,
	pub game_tag: i32,
	pub type_tag: i32,
}

#[derive(Serialize, Deserialize)]
pub struct PostMetadata {
	pub name: String,
	pub text: String,
	pub text_short: String,
	pub game_tag: i32,
	pub type_tag: i32,
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
	pub likes: i64,
	pub dislikes: i64,
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
}

#[derive(Queryable, Serialize, Deserialize, Default)]
pub struct ShortUserPosts {
	pub post: ShortPost,
	pub user: User,
}

#[derive(Queryable, Serialize, Deserialize, Default)]
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

#[derive(Insertable)]
#[diesel(table_name = reports)]
pub struct NewReport {
	pub report_id: i32,
	pub user_id: i64,
	pub post_id: i32,
	pub description: String,
}
