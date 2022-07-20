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
	schema::users::user_id,
	schema::users::user_name,
	schema::users::user_avatar
);

#[get("/robots.txt")]
pub fn robots() -> String {
	String::from("User-agent: *\nDisallow: /api/")
}

#[get("/storage/<user_id>/<file_type>/<file_name>")]
pub fn get_from_storage(
	connection: &models::ConnectionState,
	user_id: i64,
	file_type: String,
	file_name: String,
) -> Option<(Status, (ContentType, std::fs::File))> {
	let file = format!("storage/{}/{}/{}", user_id, file_type, file_name);
	if file_type == "posts" {
		let path = format!("{}/{}", models::BASE_URL.to_string(), file);
		let _ = posts::update_download_count(&mut connection.lock().unwrap(), path);
	}
	let file = std::fs::File::open(file);
	if file.is_err() {
		return None;
	}
	let file = file.unwrap();
	let content_type = match file_type.as_str() {
		"posts" => ContentType::ZIP,
		"images" => ContentType::PNG,
		_ => return None,
	};
	Some((Status::Ok, (content_type, file)))
}

#[launch]
fn rocket() -> _ {
	dotenv().ok();
	let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| String::new());
	if database_url.is_empty() {
		panic!("DATABASE_URL must not be empty");
	}
	let connection = Mutex::new(PgConnection::establish(&database_url).unwrap());
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
				get_from_storage,
				robots,
			],
		)
		.mount(
			"/api/v1/posts",
			routes![
				api::v1::posts::upload_image,
				api::v1::posts::upload_archive,
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
}
