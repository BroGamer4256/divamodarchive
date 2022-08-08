#[macro_use]
extern crate diesel;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_dyn_templates;

pub mod api;
pub mod models;
pub mod posts;
pub mod schema;
pub mod users;
pub mod web;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use rocket::http::ContentType;
use rocket::serde::{Deserialize, Serialize};
use rocket::*;
use rocket_dyn_templates::Template;
use std::env;

#[launch]
pub async fn rocket() -> _ {
	dotenv().ok();
	let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| String::new());
	assert!(!database_url.is_empty(), "DATABASE_URL must not be empty");
	let manager = diesel::r2d2::ConnectionManager::<PgConnection>::new(database_url);
	let pool = diesel::r2d2::Pool::builder().max_size(20).build(manager);
	let pool = match pool {
		Ok(pool) => pool,
		Err(err) => panic!("Failed to create database pool: {}", err),
	};

	let region_provider =
		aws_config::meta::region::RegionProviderChain::default_provider().or_else("us-west-1");
	let config = aws_config::from_env().region(region_provider).load().await;
	let s3 = aws_sdk_s3::Client::new(&config);

	rocket::build()
		.mount(
			"/",
			routes![
				web::find_posts,
				web::details,
				web::login,
				web::upload,
				web::user,
				web::edit,
				web::set_theme,
				web::dependency,
				web::dependency_add,
				web::dependency_remove,
				web::about,
				web::liked,
				web::logout,
				web::admin,
				web::remove_post_admin,
				web::remove_report,
				web::report,
				web::report_send,
				get_from_storage,
				robots,
				favicon,
				large_icon,
				sitemap,
			],
		)
		.mount(
			"/api/v1/posts",
			routes![
				api::v1::posts::upload_image,
				api::v1::posts::upload_archive,
				api::v1::posts::upload,
				api::v1::posts::edit,
				api::v1::posts::details,
				api::v1::posts::like,
				api::v1::posts::dislike,
				api::v1::posts::dependency,
				api::v1::posts::latest,
				api::v1::posts::popular,
				api::v1::posts::delete,
				api::v1::posts::posts,
				api::v1::posts::post_count,
			],
		)
		.mount(
			"/api/v1/users",
			routes![
				api::v1::users::login,
				api::v1::users::details,
				api::v1::users::latest,
				api::v1::users::popular,
				api::v1::users::delete
			],
		)
		.mount("/api/v1", routes![api::v1::get_spec])
		.manage(pool)
		.manage(s3)
		.attach(Template::fairing())
}

#[get("/robots.txt")]
pub const fn robots() -> &'static str {
	"User-agent: *\nDisallow: /api/\nSitemap: /sitemap.xml"
}

#[get("/favicon.ico")]
pub const fn favicon() -> (ContentType, &'static [u8]) {
	(
		ContentType::PNG,
		include_bytes!("../static/DMA_BLACK_STARLESS.png"),
	)
}

#[get("/large_icon.png")]
pub const fn large_icon() -> (ContentType, &'static [u8]) {
	(ContentType::PNG, include_bytes!("../static/DMA_BLACK.png"))
}

// The code ahead is very fucking jank, I'm sorry
// This handles creating a dynamic sitemap xml
// I know this is way too many structs but I can't figure out a better way
// It's not performance critical anyways so /shrug

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "loc")]
pub struct Loc {
	#[serde(rename = "$value")]
	pub loc: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "changefreq")]
pub struct Changefreq {
	#[serde(rename = "$value")]
	pub changefreq: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "priority")]
pub struct Priority {
	#[serde(rename = "$value")]
	pub priority: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "url")]
pub struct Url {
	pub loc: Loc,
	pub changefreq: Changefreq,
	pub priority: Priority,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "urlset")]
pub struct Urlset {
	pub url: Vec<Url>,
	pub xmlns: String,
}

#[get("/sitemap.xml")]
pub fn sitemap(connection: &models::ConnectionState) -> (ContentType, String) {
	let ids = posts::get_post_ids(&mut models::get_connection(connection));
	let mut urls = Vec::new();
	let base_url = Url {
		loc: Loc {
			loc: format!("{}/", *models::BASE_URL),
		},
		changefreq: Changefreq {
			changefreq: String::from("hourly"),
		},
		priority: Priority {
			priority: String::from("1.0"),
		},
	};
	urls.push(base_url);
	let about_url = Url {
		loc: Loc {
			loc: format!("{}/about", *models::BASE_URL),
		},
		changefreq: Changefreq {
			changefreq: String::from("monthly"),
		},
		priority: Priority {
			priority: String::from("0.5"),
		},
	};
	urls.push(about_url);
	for id in ids {
		let url = Url {
			loc: Loc {
				loc: format!("{}/posts/{}", *models::BASE_URL, id),
			},
			changefreq: Changefreq {
				changefreq: String::from("weekly"),
			},
			priority: Priority {
				priority: String::from("1.0"),
			},
		};
		urls.push(url);
	}
	let xml = Urlset {
		url: urls,
		xmlns: String::from("http://www.sitemaps.org/schemas/sitemap/0.9"),
	};
	let xml = quick_xml::se::to_string(&xml).unwrap();
	(ContentType::XML, xml)
}

#[get("/storage/<user_id>/<file_name>")]
pub async fn get_from_storage(
	connection: &models::ConnectionState,
	user_id: i64,
	file_name: &str,
	s3: &State<aws_sdk_s3::Client>,
) -> Option<response::Redirect> {
	let file = format!("{}/{}", user_id, file_name);
	let path = format!("{}/storage/{}", *models::BASE_URL, file);
	let _result = posts::update_download_count(&mut models::get_connection(connection), path);
	let file = s3
		.get_object()
		.bucket("divamodarchive")
		.key(file)
		.presigned(
			aws_sdk_s3::presigning::config::PresigningConfig::expires_in(
				std::time::Duration::from_secs(60 * 60 * 24),
			)
			.unwrap(),
		)
		.await;

	let file = match file {
		Ok(file) => file,
		Err(_) => return None,
	};

	Some(response::Redirect::to(file.uri().to_string()))
}

allow_columns_to_appear_in_same_group_by_clause!(
	schema::posts::post_id,
	schema::posts::post_name,
	schema::posts::post_text,
	schema::posts::post_text_short,
	schema::posts::post_image,
	schema::posts::post_images_extra,
	schema::posts::post_link,
	schema::posts::post_date,
	schema::posts::post_game_tag,
	schema::posts::post_type_tag,
	schema::users::user_id,
	schema::users::user_name,
	schema::users::user_avatar,
	schema::reports::report_id,
	schema::reports::user_id,
	schema::reports::post_id,
	schema::reports::description,
	schema::reports::time,
);
