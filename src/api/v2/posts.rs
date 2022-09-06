// Used to get details for unspecific posts with optional generic filters

use crate::models::*;
use crate::posts::*;
use rocket::fairing::*;
use rocket::http::*;
use rocket::serde::json::Json;
use rocket::*;

#[get("/count?<name>&<game_tag>")]
pub async fn count(
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
	if result.len() != 0 {
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
	if result.len() != 0 {
		Ok(Json(result))
	} else {
		Err(Status::NotFound)
	}
}

#[get("/detailed/changes?<since>")]
pub async fn changes_detailed(
	connection: &ConnectionState,
	since: time::PrimitiveDateTime,
) -> Json<Vec<DetailedPostNoDepends>> {
	let connection = &mut get_connection(connection).await;
	let posts = get_changed_posts_detailed(connection, since).await;
	Json(posts.unwrap_or(vec![]))
}

#[get("/short/changes?<since>")]
pub async fn changes_short(
	connection: &ConnectionState,
	since: time::PrimitiveDateTime,
) -> Json<Vec<ShortPost>> {
	let connection = &mut get_connection(connection).await;
	let posts = get_changed_posts_short(connection, since).await;
	Json(posts.unwrap_or(vec![]))
}

pub struct VecErrHandler;

#[rocket::async_trait]
impl Fairing for VecErrHandler {
	fn info(&self) -> Info {
		Info {
			name: "Vec Error Handler",
			kind: Kind::Response,
		}
	}
	async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
		if response.status() != Status::NotFound
			|| !request.uri().path().starts_with("/api/v2/posts/")
		{
			return;
		}

		let body = format!("[]");
		response.set_sized_body(body.len(), std::io::Cursor::new(body));
	}
}
