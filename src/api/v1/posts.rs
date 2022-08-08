use crate::models::*;
use crate::posts::*;
use rocket::http::Status;
use rocket::serde::json::Json;
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
pub async fn upload_image(user: User) -> Result<Json<String>, Status> {
	let cloudflare_url = format!(
		"https://api.cloudflare.com/client/v4/accounts/{}/images/v2/direct_upload",
		*CLOUDFLARE_ACCOUNT_ID
	);
	let params =
		reqwest::multipart::Form::new().text("metadata", format!("{{\"user\":\"{}\"}}", user.id));
	let response = reqwest::Client::new()
		.post(&cloudflare_url)
		.header(
			"Authorization",
			format!("Bearer {}", *CLOUDFLARE_IMAGE_TOKEN),
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
	if !response.success {
		Err(Status::InternalServerError)
	} else {
		Ok(Json(response.result.uploadURL))
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
) -> Result<Json<Post>, Status> {
	let post = post.into_inner();
	if !post
		.image
		.starts_with(&format!("{}/cdn-cgi/imagedelivery", *BASE_URL))
		|| !post
			.link
			.starts_with(&format!("{}/storage/{}/posts/", *BASE_URL, user.id))
		|| reqwest::get(post.image.clone()).await.is_err()
	{
		return Err(Status::BadRequest);
	}
	let post = create_post(
		&mut get_connection(connection),
		post,
		user,
		update_id.unwrap_or(-1),
	)?;
	Ok(Json(post))
}

#[post("/edit?<update_id>", data = "<post>")]
pub fn edit(
	connection: &ConnectionState,
	user: User,
	post: Json<PostMetadata>,
	update_id: i32,
) -> Result<Json<Post>, Status> {
	let post = post.into_inner();
	let connection = &mut get_connection(connection);
	if !owns_post(connection, update_id, user.id) {
		return Err(Status::Unauthorized);
	}

	let result = update_post(connection, post, update_id)?;
	Ok(Json(result))
}

#[get("/<id>")]
pub fn details(connection: &ConnectionState, id: i32) -> Result<Json<DetailedPost>, Status> {
	let result = get_post(&mut get_connection(connection), id)?;
	Ok(Json(result))
}

#[post("/<id>/like")]
pub fn like(connection: &ConnectionState, id: i32, user: User) -> Result<Json<LikedPost>, Status> {
	let result = like_post_from_ids(&mut get_connection(connection), user.id, id)?;
	Ok(Json(result))
}

#[post("/<id>/dislike")]
pub fn dislike(
	connection: &ConnectionState,
	id: i32,
	user: User,
) -> Result<Json<DislikedPost>, Status> {
	let result = dislike_post_from_ids(&mut get_connection(connection), user.id, id)?;
	Ok(Json(result))
}

// Add a dependency to the post with id on dependency
// Return the updated post
#[post("/<id>/dependency/<dependency>")]
pub fn dependency(connection: &ConnectionState, id: i32, dependency: i32, user: User) -> Status {
	let connection = &mut get_connection(connection);
	if owns_post(connection, id, user.id) {
		add_dependency(connection, id, dependency)
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
) -> (Status, Json<Vec<DetailedPost>>) {
	let result = get_latest_posts_detailed(
		&mut get_connection(connection),
		name.unwrap_or_default(),
		offset.unwrap_or(0),
		game_tag.unwrap_or(0),
		limit.unwrap_or(*WEBUI_LIMIT),
	);
	if let Ok(result) = result {
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
) -> (Status, Json<Vec<DetailedPost>>) {
	let result = get_popular_posts_detailed(
		&mut get_connection(connection),
		name.unwrap_or_default(),
		offset.unwrap_or(0),
		game_tag.unwrap_or(0),
		limit.unwrap_or(*WEBUI_LIMIT),
	);
	if let Ok(result) = result {
		(Status::Ok, Json(result))
	} else {
		(Status::NotFound, Json(vec![]))
	}
}

#[delete("/<id>/delete")]
pub fn delete(connection: &ConnectionState, id: i32, user: User) -> Status {
	let connection = &mut get_connection(connection);
	if owns_post(connection, id, user.id) {
		delete_post(connection, id)
	} else {
		Status::Forbidden
	}
}

// Usage of this is a bit weird
// /api/v1/posts/posts?post_id=1&post_id=2
// Gets the details of posts with id 1 and 2
#[get("/posts?<post_id>")]
pub fn posts(
	connection: &ConnectionState,
	post_id: Vec<i32>,
) -> (Status, Json<Vec<DetailedPostNoDepends>>) {
	let count = post_id.len();
	let result = get_posts_detailed(&mut get_connection(connection), post_id);
	if result.is_empty() {
		(Status::NotFound, Json(result))
	} else if result.len() != count {
		// (Status::PartialContent, Json(result))
		(Status::Ok, Json(result))
	} else {
		(Status::Ok, Json(result))
	}
}

#[get("/post_count?<name>&<game_tag>")]
pub fn post_count(
	connection: &ConnectionState,
	name: Option<String>,
	game_tag: Option<i32>,
) -> Json<i64> {
	Json(get_post_count(
		&mut get_connection(connection),
		name.unwrap_or_default(),
		game_tag.unwrap_or(0),
	))
}
