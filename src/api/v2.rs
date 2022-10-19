// Despite the naming convention this is not a straight upgrade to v1
// This has several important missing features mainly related to writing data
// V2 is designed specifically for apps which simply want to read posts

use crate::database::*;
use crate::models;
use crate::models::*;
use rocket::fairing::*;
use rocket::http::*;
use rocket::serde::json::Json;
use rocket::*;

#[get("/v2.json")]
pub const fn get_spec() -> &'static str {
	include_str!("v2.json")
}

// Used to get details for unspecific posts with optional generic filters

#[get("/post_count?<name>&<game_tag>")]
pub fn post_count(
	connection: &ConnectionState,
	name: Option<String>,
	game_tag: Option<i32>,
) -> Option<Json<i64>> {
	let connection = &mut get_connection(connection);
	let count = get_post_count(connection, name.unwrap_or_default(), game_tag.unwrap_or(0))?;
	Some(Json(count))
}

#[get("/detailed/latest?<name>&<offset>&<game_tag>&<limit>")]
pub fn latest_detailed(
	connection: &ConnectionState,
	name: Option<String>,
	offset: Option<i64>,
	game_tag: Option<i32>,
	limit: Option<i64>,
	config: &State<models::Config>,
) -> Option<Json<Vec<DetailedPost>>> {
	let connection = &mut get_connection(connection);
	let posts = get_latest_posts_detailed(
		connection,
		name.unwrap_or_default(),
		offset.unwrap_or(0),
		game_tag.unwrap_or(0),
		limit.unwrap_or(config.webui_limit),
	)?;
	Some(Json(posts))
}

#[get("/short/latest?<name>&<offset>&<game_tag>&<limit>")]
pub fn latest_short(
	connection: &ConnectionState,
	name: Option<String>,
	offset: Option<i64>,
	game_tag: Option<i32>,
	limit: Option<i64>,
	config: &State<models::Config>,
) -> Option<Json<Vec<ShortPost>>> {
	let connection = &mut get_connection(connection);
	let result = get_latest_posts(
		connection,
		name.unwrap_or_default(),
		offset.unwrap_or(0),
		game_tag.unwrap_or(0),
		limit.unwrap_or(config.webui_limit),
	)?;
	Some(Json(result))
}

#[get("/detailed/popular?<name>&<offset>&<game_tag>&<limit>")]
pub fn popular_detailed(
	connection: &ConnectionState,
	name: Option<String>,
	offset: Option<i64>,
	game_tag: Option<i32>,
	limit: Option<i64>,
	config: &State<models::Config>,
) -> Option<Json<Vec<DetailedPost>>> {
	let connection = &mut get_connection(connection);
	let result = get_popular_posts_detailed(
		connection,
		name.unwrap_or_default(),
		offset.unwrap_or(0),
		game_tag.unwrap_or(0),
		limit.unwrap_or(config.webui_limit),
	)?;
	Some(Json(result))
}

#[get("/short/popular?<name>&<offset>&<game_tag>&<limit>")]
pub fn popular_short(
	connection: &ConnectionState,
	name: Option<String>,
	offset: Option<i64>,
	game_tag: Option<i32>,
	limit: Option<i64>,
	config: &State<models::Config>,
) -> Option<Json<Vec<ShortPost>>> {
	let connection = &mut get_connection(connection);
	let result = get_popular_posts(
		connection,
		name.unwrap_or_default(),
		offset.unwrap_or(0),
		game_tag.unwrap_or(0),
		limit.unwrap_or(config.webui_limit),
	)?;
	Some(Json(result))
}

#[get("/detailed/changes?<since>")]
pub fn changes_detailed(
	connection: &ConnectionState,
	since: time::Date,
) -> Option<Json<Vec<DetailedPost>>> {
	let connection = &mut get_connection(connection);
	let posts = get_changed_posts_detailed(connection, since.midnight())?;
	Some(Json(posts))
}

#[get("/short/changes?<since>")]
pub fn changes_short(
	connection: &ConnectionState,
	since: time::Date,
) -> Option<Json<Vec<ShortPost>>> {
	let connection = &mut get_connection(connection);
	let posts = get_changed_posts_short(connection, since.midnight())?;
	Some(Json(posts))
}

// Used to get details for specific posts

// Usage of this is a bit weird
// /api/v2/posts?post_id=1&post_id=2
// Gets the details of posts with id 1 and 2
// Returns in order of post id ascending
#[get("/detailed/posts?<post_ids>")]
pub fn posts(
	connection: &ConnectionState,
	post_ids: Vec<i32>,
) -> Option<(Status, Json<Vec<DetailedPost>>)> {
	let count = post_ids.len();
	let connection = &mut get_connection(connection);
	let result = get_posts_detailed(connection, post_ids);
	result.map(|posts| {
		if posts.len() == count {
			(Status::Ok, Json(posts))
		} else {
			(Status::PartialContent, Json(posts))
		}
	})
}

#[get("/detailed/post/<id>")]
pub fn post_detailed(connection: &ConnectionState, id: i32) -> Option<Json<DetailedPost>> {
	let connection = &mut get_connection(connection);
	let post = get_post(connection, id)?;
	Some(Json(post))
}

#[get("/short/post/<id>")]
pub fn post_short(connection: &ConnectionState, id: i32) -> Option<Json<ShortPost>> {
	let connection = &mut get_connection(connection);
	let post = get_short_post(connection, id)?;
	Some(Json(post))
}

pub struct V2VecErrHandler;

#[rocket::async_trait]
impl Fairing for V2VecErrHandler {
	fn info(&self) -> Info {
		Info {
			name: "V2 Vec Error Handler",
			kind: Kind::Response,
		}
	}
	async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
		if response.status() != Status::NotFound || !request.uri().path().starts_with("/api/v2/") {
			return;
		}

		let body = "[]";
		response.set_sized_body(body.len(), std::io::Cursor::new(body));
		response.set_status(Status::NotFound);
	}
}
