#![feature(let_chains)]
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_dyn_templates;

pub mod api;
pub mod database;
pub mod models;
pub mod schema;
pub mod web;

use diesel::pg::PgConnection;
use dotenvy::dotenv;
use rocket::fairing::*;
use rocket::http::*;
use rocket::serde::{Deserialize, Serialize};
use rocket::*;
use rocket_dyn_templates::Template;
use std::env;
use std::time::SystemTime;

#[launch]
pub async fn rocket() -> Rocket<Build> {
	dotenv().expect(".env must exist");
	let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| String::new());
	assert!(!database_url.is_empty(), "DATABASE_URL must not be empty");
	let manager = diesel::r2d2::ConnectionManager::<PgConnection>::new(database_url);
	let pool = diesel::r2d2::Pool::builder().max_size(16).build(manager);
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
				web::login_failed,
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
				web::create_comment,
				web::reply_comment,
				web::remove_comment,
				get_from_storage,
				robots,
				favicon,
				large_icon,
				sitemap,
				flamethrower,
			],
		)
		.mount("/api/v1", routes![api::v1::get_spec])
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
		.mount(
			"/api/v2",
			routes![
				api::v2::get_spec,
				api::v2::posts,
				api::v2::post_detailed,
				api::v2::post_short,
				api::v2::post_count,
				api::v2::latest_detailed,
				api::v2::latest_short,
				api::v2::popular_detailed,
				api::v2::popular_short,
				api::v2::changes_detailed,
				api::v2::changes_short,
			],
		)
		.manage(pool)
		.manage(s3)
		.attach(Template::fairing())
		.attach(api::v2::V2VecErrHandler)
		.attach(RequestTimer)
}

pub struct RequestTimer;

#[derive(Copy, Clone)]
pub struct Timer {
	pub time: SystemTime,
}

impl Timer {
	fn new() -> Self {
		Self {
			time: SystemTime::now(),
		}
	}
}

#[rocket::async_trait]
impl Fairing for RequestTimer {
	fn info(&self) -> Info {
		Info {
			name: "Request Timer",
			kind: Kind::Request | Kind::Response,
		}
	}

	async fn on_request(&self, request: &mut Request<'_>, _: &mut Data<'_>) {
		request.local_cache(Timer::new);
	}

	async fn on_response<'r>(&self, request: &'r Request<'_>, result: &mut Response<'r>) {
		let start_time = request.local_cache(Timer::new);
		let time = start_time.time.elapsed().unwrap_or_default();
		let time_str = format!("{:.3?}", time).replace('µ', "u");
		info!("{} took {}", request.uri(), time_str);
		result.set_raw_header("Time-Spent", time_str);
	}
}

#[get("/flamethrower.min.js")]
pub const fn flamethrower() -> (ContentType, &'static str) {
	(
		ContentType::JavaScript,
		include_str!("../static/flamethrower.min.js"),
	)
}

#[get("/robots.txt")]
pub fn robots() -> String {
	format!(
		"User-agent: *\nDisallow: /api/\nSitemap: {}/sitemap.xml",
		*models::BASE_URL
	)
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
#[serde(rename = "lastmod")]
pub struct Lastmod {
	#[serde(rename = "$value")]
	pub lastmod: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "url")]
pub struct Url {
	pub loc: Loc,
	pub changefreq: Changefreq,
	pub priority: Priority,
	pub lastmod: Option<Lastmod>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "urlset")]
pub struct Urlset {
	pub url: Vec<Url>,
	pub xmlns: String,
}

#[get("/sitemap.xml")]
pub fn sitemap(connection: &models::ConnectionState) -> Option<(ContentType, String)> {
	let mut urls = Vec::new();
	let connection = &mut models::get_connection(connection);
	let latest_date = database::get_post_latest_date(connection)?;
	let base_url = Url {
		loc: Loc {
			loc: format!("{}/", *models::BASE_URL),
		},
		changefreq: Changefreq {
			changefreq: String::from("daily"),
		},
		priority: Priority {
			priority: String::from("1.0"),
		},
		lastmod: Some(Lastmod {
			lastmod: latest_date.date().to_string(),
		}),
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
		lastmod: None,
	};
	let posts_info = database::get_post_ids(connection)?;
	urls.push(about_url);
	for post_info in posts_info {
		let url = Url {
			loc: Loc {
				loc: format!("{}/posts/{}", *models::BASE_URL, post_info.id),
			},
			changefreq: Changefreq {
				changefreq: String::from("monthly"),
			},
			priority: Priority {
				priority: String::from("1.0"),
			},
			lastmod: Some(Lastmod {
				lastmod: post_info.date.date().to_string(),
			}),
		};
		urls.push(url);
	}
	let xml = Urlset {
		url: urls,
		xmlns: String::from("http://www.sitemaps.org/schemas/sitemap/0.9"),
	};
	let xml = quick_xml::se::to_string(&xml).ok()?;
	Some((ContentType::XML, xml))
}

#[get("/storage/<user_id>/<file_name>")]
pub async fn get_from_storage(
	connection: &models::ConnectionState,
	user_id: i64,
	file_name: &str,
	s3: &State<aws_sdk_s3::Client>,
	ip: models::HttpIp,
) -> Result<response::Redirect, Status> {
	let connection = &mut models::get_connection(connection);
	let file = format!("{}/{}", user_id, file_name);
	let path = format!(
		"{}/storage/{}/{}",
		*models::BASE_URL,
		user_id,
		urlencoding::encode(file_name)
	);
	let file_size = s3
		.head_object()
		.bucket("divamodarchive")
		.key(file.clone())
		.send()
		.await;

	let file_size = match file_size {
		Ok(file_size) => file_size.content_length(),
		Err(_) => return Err(Status::NotFound),
	};

	let result = database::update_download_limit(connection, ip.ip, file_size);
	if result.is_failure() {
		return Err(result);
	}
	_ = database::update_download_count(connection, path);

	let file = s3
		.get_object()
		.bucket("divamodarchive")
		.key(file)
		.presigned(
			aws_sdk_s3::presigning::config::PresigningConfig::expires_in(
				std::time::Duration::from_secs(60 * 10),
			)
			.unwrap(),
		)
		.await;

	let file = match file {
		Ok(file) => file,
		Err(_) => return Err(Status::NotFound),
	};

	Ok(response::Redirect::to(file.uri().to_string()))
}

pub trait DidSucceed {
	fn is_success(&self) -> bool;
	fn is_failure(&self) -> bool;
}

impl DidSucceed for Status {
	fn is_success(&self) -> bool {
		self.class() == StatusClass::Success
	}

	fn is_failure(&self) -> bool {
		self.class() != StatusClass::Success
	}
}
