use crate::database::*;
use crate::models::*;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use serde::Deserialize;
use serde::Serialize;

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
struct CloudflareDirectUploadResult {
	id: String,
	uploadURL: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct CloudflareDirectUpload {
	success: bool,
	result: CloudflareDirectUploadResult,
}

#[get("/upload_image")]
pub async fn upload_image(user: User, config: &State<Config>) -> Result<Json<String>, Status> {
	let cloudflare_url = format!(
		"https://api.cloudflare.com/client/v4/accounts/{}/images/v2/direct_upload",
		config.cloudflare_account_id
	);
	let params =
		reqwest::multipart::Form::new().text("metadata", format!("{{\"user\":\"{}\"}}", user.id));
	let response = reqwest::Client::new()
		.post(&cloudflare_url)
		.header(
			"Authorization",
			format!("Bearer {}", config.cloudflare_image_token),
		)
		.multipart(params)
		.send()
		.await;

	let response = match response {
		Ok(response) => response,
		Err(_) => return Err(Status::InternalServerError),
	};
	if !response.status().is_success() {
		return Err(Status::InternalServerError);
	}
	let response: Result<CloudflareDirectUpload, _> = response.json().await;
	let response = match response {
		Ok(response) => response,
		Err(_) => return Err(Status::InternalServerError),
	};
	if response.success {
		Ok(Json(response.result.uploadURL))
	} else {
		Err(Status::InternalServerError)
	}
}

// Return signed URL that allows javascript frontend to upload file to S3 bucket
#[get("/upload_archive?<name>")]
pub async fn upload_archive(
	user: User,
	s3: &rocket::State<aws_sdk_s3::Client>,
	name: String,
) -> Result<Json<String>, Status> {
	let url = s3
		.put_object()
		.bucket("divamodarchive")
		.key(format!("{}/{}", user.id, name))
		.presigned(
			aws_sdk_s3::presigning::config::PresigningConfig::expires_in(
				std::time::Duration::from_secs(60 * 60 * 24),
			)
			.unwrap(),
		)
		.await;

	match url {
		Ok(url) => Ok(Json(url.uri().to_string())),
		Err(_) => Err(Status::InternalServerError),
	}
}

#[post("/upload?<update_id>", data = "<post>")]
pub async fn upload(
	connection: &ConnectionState,
	user: User,
	post: Json<PostUnidentified>,
	update_id: Option<i32>,
	config: &State<Config>,
) -> Result<Json<Post>, Status> {
	let post = post.into_inner();
	if update_id.is_none() && post.image.is_none() {
		return Err(Status::BadRequest);
	}
	if let Some(image) = post.image.clone() && (!image.starts_with(&format!("{}/cdn-cgi/imagedelivery", config.base_url)) || reqwest::get(image).await.is_err()){
		return Err(Status::BadRequest);
	}
	if !post
		.link
		.starts_with(&format!("{}/storage/{}/", config.base_url, user.id))
	{
		return Err(Status::BadRequest);
	}
	let connection = &mut get_connection(connection);
	let change = post.change.clone();
	let change_download = post.change_download.clone();
	let new_post = create_post(connection, post, user, update_id.unwrap_or(-1))?;
	if let Some(change) = change {
		add_changelog(connection, new_post.id, change, change_download);
	}
	Ok(Json(new_post))
}

#[post("/edit?<update_id>", data = "<post>")]
pub fn edit(
	connection: &ConnectionState,
	user: User,
	post: Json<PostMetadata>,
	update_id: i32,
) -> Option<Json<Post>> {
	let post = post.into_inner();
	let connection = &mut get_connection(connection);
	if !owns_post(connection, update_id, user.id) {
		return None;
	}

	let change = post.change.clone();
	let result = update_post(connection, post, update_id);
	let post = match result {
		Some(post) => post,
		None => return None,
	};
	if let Some(change) = change {
		add_changelog(connection, update_id, change, None);
	}
	Some(Json(post))
}

#[get("/<id>")]
pub fn details(connection: &ConnectionState, id: i32) -> Option<Json<DetailedPost>> {
	let connection = &mut get_connection(connection);
	let result = get_post(connection, id)?;
	Some(Json(result))
}

#[post("/<id>/like")]
pub fn like(connection: &ConnectionState, id: i32, user: User) -> Option<Json<LikedPost>> {
	let connection = &mut get_connection(connection);
	let result = like_post_from_ids(connection, user.id, id)?;
	Some(Json(result))
}

#[post("/<id>/dislike")]
pub fn dislike(connection: &ConnectionState, id: i32, user: User) -> Option<Json<DislikedPost>> {
	let connection = &mut get_connection(connection);
	let result = dislike_post_from_ids(connection, user.id, id)?;
	Some(Json(result))
}

// Add a dependency to the post with id on dependency
// Return the updated post
#[post("/<id>/dependency/<dependency>")]
pub fn dependency(connection: &ConnectionState, id: i32, dependency: i32, user: User) -> Status {
	let connection = &mut get_connection(connection);
	if owns_post(connection, id, user.id) {
		if add_dependency(connection, id, dependency) {
			Status::Ok
		} else {
			Status::NotFound
		}
	} else {
		Status::Forbidden
	}
}

#[get("/latest?<name>&<offset>&<game_tag>&<limit>")]
pub fn latest(
	connection: &ConnectionState,
	name: Option<String>,
	offset: Option<i64>,
	game_tag: Option<i32>,
	limit: Option<i64>,
	config: &State<Config>,
) -> (Status, Json<Vec<DetailedPost>>) {
	let connection = &mut get_connection(connection);
	let result = get_latest_posts_detailed(
		connection,
		name.unwrap_or_default(),
		offset.unwrap_or(0),
		game_tag.unwrap_or(0),
		limit.unwrap_or(config.webui_limit),
	);
	if let Some(result) = result {
		(Status::Ok, Json(result))
	} else {
		(Status::NotFound, Json(vec![]))
	}
}

#[get("/popular?<name>&<offset>&<game_tag>&<limit>")]
pub fn popular(
	connection: &ConnectionState,
	name: Option<String>,
	offset: Option<i64>,
	game_tag: Option<i32>,
	limit: Option<i64>,
	config: &State<Config>,
) -> (Status, Json<Vec<DetailedPost>>) {
	let connection = &mut get_connection(connection);
	let result = get_popular_posts_detailed(
		connection,
		name.unwrap_or_default(),
		offset.unwrap_or(0),
		game_tag.unwrap_or(0),
		limit.unwrap_or(config.webui_limit),
	);
	if let Some(result) = result {
		(Status::Ok, Json(result))
	} else {
		(Status::NotFound, Json(vec![]))
	}
}

#[delete("/<id>/delete")]
pub fn delete(connection: &ConnectionState, id: i32, user: User) -> Status {
	let connection = &mut get_connection(connection);
	if owns_post(connection, id, user.id) {
		delete_post(connection, id);
		Status::Ok
	} else {
		Status::Forbidden
	}
}

// Usage of this is a bit weird
// /api/v1/posts/posts?post_id=1&post_id=2
// Gets the details of posts with id 1 and 2
// Returns in order of post id ascending
#[get("/posts?<post_id>")]
pub fn posts(connection: &ConnectionState, post_id: Vec<i32>) -> (Status, Json<Vec<DetailedPost>>) {
	let count = post_id.len();
	let connection = &mut get_connection(connection);
	let result = get_posts_detailed(connection, post_id);
	match result {
		Some(posts) => {
			if posts.len() == count {
				(Status::Ok, Json(posts))
			} else {
				(Status::PartialContent, Json(posts))
			}
		}
		None => (Status::NotFound, Json(vec![])),
	}
}

#[get("/post_count?<name>&<game_tag>")]
pub fn post_count(
	connection: &ConnectionState,
	name: Option<String>,
	game_tag: Option<i32>,
) -> Json<i64> {
	let connection = &mut get_connection(connection);
	Json(get_post_count(connection, name.unwrap_or_default(), game_tag.unwrap_or(0)).unwrap_or(0))
}
