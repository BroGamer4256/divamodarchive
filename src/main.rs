pub mod api;
pub mod models;
pub mod sitemap;
pub mod web;

use axum::{http::HeaderMap, routing::*, Router};
use meilisearch_sdk::indexes::*;
use models::*;
use sqlx::postgres::PgPoolOptions;

#[derive(Clone)]
pub struct Config {
	pub decoding_key: jsonwebtoken::DecodingKey,
	pub encoding_key: jsonwebtoken::EncodingKey,
	pub discord_id: String,
	pub discord_secret: String,
	pub cloudflare_image_token: String,
	pub cloudflare_account_id: String,
	pub admins: Vec<i64>,
}

#[derive(Clone)]
pub struct AppState {
	pub config: Config,
	pub db: sqlx::Pool<sqlx::Postgres>,
	pub meilisearch: Index,
}

#[tokio::main]
async fn main() {
	dotenvy::dotenv().expect(".env must exist");

	let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must exist");
	let db = PgPoolOptions::new()
		.max_connections(32)
		.connect(&database_url)
		.await
		.expect("Could not connect to database");
	sqlx::migrate!()
		.run(&db)
		.await
		.expect("Unable to run migrations");

	let secret_key = std::env::var("SECRET_KEY").expect("SECRET_KEY must exist");
	let decoding_key = jsonwebtoken::DecodingKey::from_secret(secret_key.as_bytes());
	let encoding_key = jsonwebtoken::EncodingKey::from_secret(secret_key.as_bytes());

	let discord_id = std::env::var("DISCORD_ID").expect("DISCORD_ID must exist");
	let discord_secret = std::env::var("DISCORD_SECRET").expect("DISCORD_SECRET must exist");

	let cloudflare_image_token =
		std::env::var("CLOUDFLARE_IMAGE_TOKEN").expect("CLOUDFLARE_IMAGE_TOKEN must exist");
	let cloudflare_account_id =
		std::env::var("CLOUDFLARE_ACCOUNT_ID").expect("CLOUDFLARE_ACCOUNT_ID must exist");

	let admins = std::env::var("ADMIN_IDS")
		.expect("ADMIN_IDS must exist")
		.split(',')
		.map(|x| x.parse::<i64>().expect("Admin IDs must be i64"))
		.collect();

	let config = Config {
		decoding_key,
		encoding_key,
		discord_id,
		discord_secret,
		cloudflare_image_token,
		cloudflare_account_id,
		admins,
	};

	let client = meilisearch_sdk::client::Client::new("http://0.0.0.0:7700", None::<&str>).unwrap();
	let meilisearch = client.index("posts");

	let posts = sqlx::query!("SELECT id FROM posts").fetch_all(&db).await;
	if let Ok(posts) = posts {
		let mut vec = Vec::with_capacity(posts.len());
		for post in &posts {
			let Some(post) = Post::get_short(post.id, &db).await else {
				continue;
			};
			vec.push(post);
		}
		meilisearch.add_or_update(&vec, None).await.unwrap();
	}

	meilisearch
		.set_searchable_attributes(&["name", "text", "authors.name"])
		.await
		.unwrap();

	meilisearch
		.set_filterable_attributes(&["post_type", "explicit"])
		.await
		.unwrap();

	meilisearch
		.set_sortable_attributes(&["download_count", "like_count", "time"])
		.await
		.unwrap();

	let state = AppState {
		config,
		db,
		meilisearch,
	};
	let router = Router::new()
		.route("/robots.txt", get(robots))
		.route("/favicon.ico", get(favicon))
		.route("/large_icon.png", get(large_icon))
		.route("/sitemap.xml", get(sitemap::sitemap))
		.route("/login", get(login))
		.layer(axum::extract::DefaultBodyLimit::disable())
		.with_state(state.clone())
		.merge(web::route(state.clone()))
		.merge(api::route(state.clone()));
	let listener = tokio::net::TcpListener::bind("0.0.0.0:7001")
		.await
		.expect("Unable to bind on port {}");
	axum::serve(listener, router).await.unwrap();
}

pub async fn robots() -> &'static str {
	"User-agent: *\nDisallow: /api/\nSitemap: https://divamodarchive.com/sitemap.xml"
}

pub async fn favicon() -> (HeaderMap, &'static [u8]) {
	let mut headers = HeaderMap::new();
	headers.insert("content-type", "image/vnd.microsoft.icon".parse().unwrap());
	(headers, include_bytes!("../static/DMA_BLACK_STARLESS.ico"))
}

pub async fn large_icon() -> (HeaderMap, &'static [u8]) {
	let mut headers = HeaderMap::new();
	headers.insert("content-type", "image/png".parse().unwrap());
	(headers, include_bytes!("../static/DMA_BLACK.png"))
}
