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
use rocket::http::{ContentType, Status};
use rocket::*;
use rocket_dyn_templates::Template;
use std::env;
use std::sync::Mutex;

// Why do these get deleted from the schema with migration 4?
joinable!(schema::users_disliked_posts -> schema::posts (post_id));
joinable!(schema::users_liked_posts -> schema::posts (post_id));

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

#[must_use]
#[get("/robots.txt")]
pub const fn robots() -> &'static str {
	"User-agent: *\nDisallow: /api/"
}

#[must_use]
#[get("/favicon.ico")]
pub const fn favicon() -> (ContentType, &'static [u8]) {
	(
		ContentType::PNG,
		include_bytes!("../static/DMA_BLACK_STARLESS.png"),
	)
}

#[must_use]
#[get("/large_icon.png")]
pub const fn large_icon() -> (ContentType, &'static [u8]) {
	(ContentType::PNG, include_bytes!("../static/DMA_BLACK.png"))
}

#[must_use]
#[get("/sitemap.xml")]
pub const fn sitemap() -> (ContentType, &'static [u8]) {
	(ContentType::XML, include_bytes!("../static/sitemap.xml"))
}

#[get("/storage/<user_id>/<file_type>/<file_name>")]
pub fn get_from_storage(
	connection: &models::ConnectionState,
	user_id: i64,
	file_type: &str,
	file_name: &str,
) -> Option<(Status, (ContentType, std::fs::File))> {
	let file = format!("storage/{}/{}/{}", user_id, file_type, file_name);
	if file_type == "posts" {
		let path = format!("{}/{}", *models::BASE_URL, file);
		let _ = posts::update_download_count(&mut connection.lock().unwrap(), path);
	}
	let file = std::fs::File::open(file);
	if file.is_err() {
		return None;
	}
	let file = file;
	if let Ok(file) = file {
		let content_type = match file_type {
			"posts" => ContentType::ZIP,
			"images" => ContentType::PNG,
			_ => return None,
		};
		Some((Status::Ok, (content_type, file)))
	} else {
		None
	}
}

// Rockets macros give clippy an aneurysm here, disable no_effect_underscore_binding
#[launch]
pub fn rocket() -> _ {
	dotenv().ok();
	let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| String::new());
	assert!(!database_url.is_empty(), "DATABASE_URL must not be empty");
	let connection = PgConnection::establish(&database_url);
	if let Ok(connection) = connection {
		let connection = Mutex::new(connection);

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
					api::v1::posts::upload_archive_chunk,
					api::v1::posts::finish_upload_archive_chunk,
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
			.manage(connection)
			.attach(Template::fairing())
	} else {
		panic!("Failed to connect to database");
	}
}
