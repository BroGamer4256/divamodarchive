use super::schema::*;
use dotenv::dotenv;
use jsonwebtoken::*;
use lazy_static::lazy_static;
use rocket::{
	http::Status,
	request::{FromRequest, Outcome},
	serde::{Deserialize, Serialize},
};
use std::env;

lazy_static! {
	pub static ref DECODE_KEY: DecodingKey = {
		dotenv().ok();
		let secret = env::var("SECRET_KEY").unwrap_or_else(|_| String::new());
		if secret.is_empty() {
			panic!("SECRET_KEY must not be empty");
		}
		DecodingKey::from_secret(secret.as_bytes())
	};
	pub static ref ENCODE_KEY: EncodingKey = {
		dotenv().ok();
		let secret = env::var("SECRET_KEY").unwrap_or_else(|_| String::new());
		if secret.is_empty() {
			panic!("SECRET_KEY must not be empty");
		}
		EncodingKey::from_secret(secret.as_bytes())
	};
	pub static ref DISCORD_ID: String = {
		dotenv().ok();
		let discord_id = env::var("DISCORD_ID").unwrap_or_else(|_| String::new());
		if discord_id.is_empty() {
			panic!("DISCORD_ID must not be empty");
		}
		discord_id
	};
	pub static ref DISCORD_SECRET: String = {
		dotenv().ok();
		let discord_secret = env::var("DISCORD_SECRET").unwrap_or_else(|_| String::new());
		if discord_secret.is_empty() {
			panic!("DISCORD_SECRET must not be empty");
		}
		discord_secret
	};
	pub static ref BASE_URL: String = {
		dotenv().ok();
		let base_url = env::var("BASE_URL").unwrap_or_else(|_| String::new());
		if base_url.is_empty() {
			panic!("BASE_URL must not be empty");
		}
		base_url
	};
	pub static ref MAX_IMAGE_SIZE: u64 = {
		dotenv().ok();
		let size = env::var("MAX_IMAGE_SIZE").unwrap_or_else(|_| String::new());
		if size.is_empty() {
			panic!("MAX_IMAGE_SIZE must not be empty");
		}
		size.parse::<u64>().unwrap()
	};
	pub static ref MAX_FILE_SIZE: u64 = {
		dotenv().ok();
		let size = env::var("MAX_FILE_SIZE").unwrap_or_else(|_| String::new());
		if size.is_empty() {
			panic!("MAX_FILE_SIZE must not be empty");
		}
		size.parse::<u64>().unwrap()
	};
}

pub type ConnectionState = rocket::State<std::sync::Mutex<diesel::PgConnection>>;

#[derive(Serialize, Deserialize)]
pub struct Token {
	pub iat: i64,
	pub exp: i64,
	pub user_id: i64,
}

pub fn create_jwt(user_id: i64) -> String {
	let time = chrono::offset::Utc::now().timestamp();
	let token_data = Token {
		iat: time,
		exp: time + 604800,
		user_id,
	};
	encode(&Header::default(), &token_data, &ENCODE_KEY).unwrap()
}

#[derive(Debug)]
pub enum GenericErorr {
	Missing,
	Invalid,
}

pub struct Verified {}

impl Verified {
	pub fn verify(token: &str) -> Outcome<Self, GenericErorr> {
		let token = decode::<Token>(&token, &DECODE_KEY, &Validation::default());
		if token.is_err() {
			Outcome::Failure((Status::Unauthorized, GenericErorr::Invalid))
		} else {
			Outcome::Success(Verified {})
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
		let token_data = decode::<Token>(&token, &DECODE_KEY, &Validation::default());
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

#[derive(Serialize, Deserialize)]
pub struct PostUnidentified {
	pub name: String,
	pub text: String,
	pub text_short: String,
	pub image: String,
	pub link: String,
}

#[derive(Serialize, Deserialize)]
pub struct ShortPost {
	pub id: i32,
	pub name: String,
	pub text_short: String,
	pub image: String,
	pub likes: i64,
	pub dislikes: i64,
}

#[derive(Serialize, Deserialize)]
pub struct DetailedPostNoUser {
	pub id: i32,
	pub name: String,
	pub text: String,
	pub text_short: String,
	pub image: String,
	pub link: String,
	pub likes: i64,
	pub dislikes: i64,
}

#[derive(Queryable, Serialize, Deserialize)]
pub struct DetailedPost {
	pub id: i32,
	pub name: String,
	pub text: String,
	pub text_short: String,
	pub image: String,
	pub link: String,
	pub likes: i64,
	pub dislikes: i64,
	pub user: User,
}

#[derive(Serialize, Deserialize)]
pub struct PostWithUser {
	pub id: i32,
	pub name: String,
	pub text: String,
	pub image: String,
	pub link: String,
	pub likes: i64,
	pub dislikes: i64,
	pub user: User,
}

#[derive(Serialize, Deserialize, Default)]
pub struct UserPosts {
	pub user: User,
	pub posts: Vec<ShortPost>,
}

#[derive(Serialize, Deserialize)]
pub struct UserPostsDetailed {
	pub user: User,
	pub posts: Vec<DetailedPostNoUser>,
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
	pub uploader: i64,
	pub link: String,
}

#[derive(Insertable)]
#[diesel(table_name = posts)]
pub struct NewPost<'a> {
	pub post_name: &'a str,
	pub post_text: &'a str,
	pub post_text_short: &'a str,
	pub post_image: &'a str,
	pub post_uploader: i64,
	pub post_link: &'a str,
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
