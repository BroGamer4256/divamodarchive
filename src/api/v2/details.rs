// Used to get details for specific posts

use crate::models::*;
use crate::posts::*;
use rocket::http::Status;
use rocket::serde::json::Json;

// Usage of this is a bit weird
// /api/v2/details/posts?post_id=1&post_id=2
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

#[get("/detailed/<id>")]
pub async fn detailed(connection: &ConnectionState, id: i32) -> Result<Json<DetailedPost>, Status> {
	let connection = &mut get_connection(connection).await;
	let result = get_post(connection, id).await;
	match result {
		Ok(post) => Ok(Json(post)),
		Err(status) => Err(status),
	}
}

#[get("/short/<id>")]
pub async fn short(connection: &ConnectionState, id: i32) -> Result<Json<ShortPost>, Status> {
	let connection = &mut get_connection(connection).await;
	let result = get_short_post(connection, id).await;
	match result {
		Some(post) => Ok(Json(post)),
		None => Err(Status::NotFound),
	}
}
