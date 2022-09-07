// Despite the naming convention this is not a straight upgrade to v1
// This has several important missing features mainly related to writing data
// V2 is designed specifically for apps which simply want to read posts

use crate::database::*;
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
pub async fn post_count(
	connection: &ConnectionState,
	name: Option<String>,
	game_tag: Option<i32>,
) -> Json<i64> {
	let connection = &mut get_connection(connection).await;
	Json(get_post_count(connection, name.unwrap_or_default(), game_tag.unwrap_or(0)).await)
}

#[get("/detailed/latest?<name>&<offset>&<game_tag>&<limit>")]
pub async fn latest_detailed(
	connection: &ConnectionState,
	name: Option<String>,
	offset: Option<i64>,
	game_tag: Option<i32>,
	limit: Option<i64>,
) -> Result<Json<Vec<DetailedPost>>, Status> {
	let connection = &mut get_connection(connection).await;
	let result = get_latest_posts_detailed(
		connection,
		name.unwrap_or_default(),
		offset.unwrap_or(0),
		game_tag.unwrap_or(0),
		limit.unwrap_or(*WEBUI_LIMIT),
	)
	.await;
	match result {
		Ok(posts) => Ok(Json(posts)),
		Err(status) => Err(status),
	}
}

#[get("/short/latest?<name>&<offset>&<game_tag>&<limit>")]
pub async fn latest_short(
	connection: &ConnectionState,
	name: Option<String>,
	offset: Option<i64>,
	game_tag: Option<i32>,
	limit: Option<i64>,
) -> Result<Json<Vec<ShortPost>>, Status> {
	let connection = &mut get_connection(connection).await;
	let result = get_latest_posts(
		connection,
		name.unwrap_or_default(),
		offset.unwrap_or(0),
		game_tag.unwrap_or(0),
		limit.unwrap_or(*WEBUI_LIMIT),
	)
	.await;
	if !result.is_empty() {
		Ok(Json(result))
	} else {
		Err(Status::NotFound)
	}
}

#[get("/detailed/popular?<name>&<offset>&<game_tag>&<limit>")]
pub async fn popular_detailed(
	connection: &ConnectionState,
	name: Option<String>,
	offset: Option<i64>,
	game_tag: Option<i32>,
	limit: Option<i64>,
) -> Result<Json<Vec<DetailedPost>>, Status> {
	let connection = &mut get_connection(connection).await;
	let result = get_popular_posts_detailed(
		connection,
		name.unwrap_or_default(),
		offset.unwrap_or(0),
		game_tag.unwrap_or(0),
		limit.unwrap_or(*WEBUI_LIMIT),
	)
	.await;
	match result {
		Ok(posts) => Ok(Json(posts)),
		Err(status) => Err(status),
	}
}

#[get("/short/popular?<name>&<offset>&<game_tag>&<limit>")]
pub async fn popular_short(
	connection: &ConnectionState,
	name: Option<String>,
	offset: Option<i64>,
	game_tag: Option<i32>,
	limit: Option<i64>,
) -> Result<Json<Vec<ShortPost>>, Status> {
	let connection = &mut get_connection(connection).await;
	let result = get_popular_posts(
		connection,
		name.unwrap_or_default(),
		offset.unwrap_or(0),
		game_tag.unwrap_or(0),
		limit.unwrap_or(*WEBUI_LIMIT),
	)
	.await;
	if !result.is_empty() {
		Ok(Json(result))
	} else {
		Err(Status::NotFound)
	}
}

#[get("/detailed/changes?<since>")]
pub async fn changes_detailed(
	connection: &ConnectionState,
	since: time::Date,
) -> Json<Vec<DetailedPostNoDepends>> {
	let connection = &mut get_connection(connection).await;
	let posts = get_changed_posts_detailed(connection, since.midnight()).await;
	Json(posts.unwrap_or_default())
}

#[get("/short/changes?<since>")]
pub async fn changes_short(
	connection: &ConnectionState,
	since: time::Date,
) -> Json<Vec<ShortPost>> {
	let connection = &mut get_connection(connection).await;
	let posts = get_changed_posts_short(connection, since.midnight()).await;
	Json(posts.unwrap_or_default())
}

// Used to get details for specific posts

// Usage of this is a bit weird
// /api/v2/posts?post_id=1&post_id=2
// Gets the details of posts with id 1 and 2
// Returns in order of post id ascending
#[get("/posts?<post_ids>")]
pub async fn posts(
	connection: &ConnectionState,
	post_ids: Vec<i32>,
) -> (Status, Json<Vec<DetailedPostNoDepends>>) {
	let count = post_ids.len();
	let connection = &mut get_connection(connection).await;
	let result = get_posts_detailed(connection, post_ids).await;
	if result.is_empty() {
		(Status::NotFound, Json(result))
	} else if result.len() != count {
		(Status::PartialContent, Json(result))
	} else {
		(Status::Ok, Json(result))
	}
}

// See posts function for usage details
#[get("/update_dates?<post_ids>")]
pub async fn update_dates(
	connection: &ConnectionState,
	post_ids: Vec<i32>,
) -> (Status, Json<Vec<PostUpdateTime>>) {
	let count = post_ids.len();
	let connection = &mut get_connection(connection).await;
	let result = get_update_dates(connection, post_ids).await;
	match result {
		Some(post_update_dates) => {
			let status = if post_update_dates.len() == count {
				Status::PartialContent
			} else {
				Status::Ok
			};
			(status, Json(post_update_dates))
		}
		None => (Status::NotFound, Json(vec![])),
	}
}

#[get("/post/detailed/<id>")]
pub async fn post_detailed(
	connection: &ConnectionState,
	id: i32,
) -> Result<Json<DetailedPost>, Status> {
	let connection = &mut get_connection(connection).await;
	let result = get_post(connection, id).await;
	match result {
		Ok(post) => Ok(Json(post)),
		Err(status) => Err(status),
	}
}

#[get("/post/short/<id>")]
pub async fn post_short(connection: &ConnectionState, id: i32) -> Result<Json<ShortPost>, Status> {
	let connection = &mut get_connection(connection).await;
	let result = get_short_post(connection, id).await;
	match result {
		Some(post) => Ok(Json(post)),
		None => Err(Status::NotFound),
	}
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
	}
}
