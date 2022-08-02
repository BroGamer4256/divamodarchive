use std::fs::File;
use std::io::Read;
use std::io::Write;

use crate::models::*;
use crate::posts::*;
use rocket::fs::TempFile;
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
	if let Ok(response) = response {
		if response.status().is_success() {
			let response: Result<CloudflareDirectUpload, _> = response.json().await;
			if let Ok(response) = response {
				if response.success {
					return Ok(Json(response.result.uploadURL));
				}
			}
		}
	}
	Err(Status::InternalServerError)
}

#[post("/upload_archive_chunk?<name>&<chunk>", data = "<archive_chunk>")]
pub async fn upload_archive_chunk(
	mut archive_chunk: TempFile<'_>,
	name: String,
	chunk: u32,
	user: User,
) -> Status {
	let result = std::fs::create_dir_all(format!("storage/{}/posts/{}_chunks", user.id, name));
	if result.is_err() {
		return Status::InternalServerError;
	}
	let result = archive_chunk
		.persist_to(format!(
			"storage/{}/posts/{}_chunks/{}",
			user.id, name, chunk
		))
		.await;
	if result.is_err() {
		Status::InternalServerError
	} else {
		Status::Ok
	}
}

// For this function, fuck rewriting it to remove unwraps
#[post("/finish_upload_archive_chunk?<name>")]
pub fn finish_upload_archive_chunk(name: String, user: User) -> Result<Json<String>, Status> {
	let merged_file = File::create(format!("storage/{}/posts/{}", user.id, name));
	if merged_file.is_err() {
		return Err(Status::InternalServerError);
	}
	let mut merged_file = merged_file.unwrap();
	let files = std::fs::read_dir(format!("storage/{}/posts/{}_chunks", user.id, name));
	if files.is_err() {
		return Err(Status::InternalServerError);
	}
	let files = files
		.unwrap()
		.map(|res| res.map(|e| e.path()))
		.collect::<Result<Vec<_>, std::io::Error>>();
	if files.is_err() {
		return Err(Status::InternalServerError);
	}
	let mut files = files.unwrap();

	// Sort files numerically
	files.sort_by(|a, b| {
		let a: &u32 = &a
			.file_name()
			.unwrap_or_default()
			.to_str()
			.unwrap_or_default()
			.parse()
			.unwrap_or_default();
		let b: &u32 = &b
			.file_name()
			.unwrap_or_default()
			.to_str()
			.unwrap_or_default()
			.parse()
			.unwrap_or_default();
		a.cmp(b)
	});

	for entry in files {
		let file = File::open(entry);
		if file.is_err() {
			return Err(Status::InternalServerError);
		}
		let mut file = file.unwrap();
		let mut buffer = [0u8; 1024];
		loop {
			let read = file.read(&mut buffer);
			if read.is_err() {
				return Err(Status::InternalServerError);
			}
			let read = read.unwrap();
			if read == 0 {
				break;
			}
			let result = merged_file.write_all(&buffer[..read]);
			if result.is_err() {
				return Err(Status::InternalServerError);
			}
		}
	}
	let _result = std::fs::remove_dir_all(format!("storage/{}/posts/{}_chunks", user.id, name));
	Ok(Json(format!(
		"{}/storage/{}/posts/{}",
		*BASE_URL, user.id, name
	)))
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
		|| reqwest::get(post.link.clone()).await.is_err()
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
#[must_use]
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
) -> Result<Json<Vec<DetailedPost>>, Status> {
	let result = get_latest_posts_detailed(
		&mut get_connection(connection),
		name.unwrap_or_default(),
		offset.unwrap_or(0),
		game_tag.unwrap_or(0),
		limit.unwrap_or(*WEBUI_LIMIT),
	)?;
	Ok(Json(result))
}

#[get("/popular?<name>&<offset>&<game_tag>&<limit>")]
pub fn popular(
	connection: &ConnectionState,
	name: Option<String>,
	offset: Option<i64>,
	game_tag: Option<i32>,
	limit: Option<i64>,
) -> Result<Json<Vec<DetailedPost>>, Status> {
	let result = get_popular_posts_detailed(
		&mut get_connection(connection),
		name.unwrap_or_default(),
		offset.unwrap_or(0),
		game_tag.unwrap_or(0),
		limit.unwrap_or(*WEBUI_LIMIT),
	)?;
	Ok(Json(result))
}

#[must_use]
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
#[must_use]
#[get("/posts?<post_id>")]
pub fn posts(connection: &ConnectionState, post_id: Vec<i32>) -> Json<Vec<DetailedPost>> {
	let connection = &mut get_connection(connection);
	let mut result = Vec::new();
	for id in post_id {
		if let Ok(post) = get_post(connection, id) {
			result.push(post);
		}
	}
	Json(result)
}

#[must_use]
#[get("/post_count?<name>&<game_tag>")]
pub fn post_count(
	connection: &ConnectionState,
	name: Option<String>,
	game_tag: Option<i32>,
) -> Json<i64> {
	let connection = &mut get_connection(connection);
	Json(get_post_count(
		connection,
		name.unwrap_or_default(),
		game_tag.unwrap_or(0),
	))
}
