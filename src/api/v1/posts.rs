use std::fs::File;
use std::io::Read;
use std::io::Write;

use crate::models::*;
use crate::posts::*;
use rocket::data::{Capped, Data, ToByteUnit, N};
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
		CLOUDFLARE_ACCOUNT_ID.to_string()
	);
	let params =
		reqwest::multipart::Form::new().text("metadata", format!("{{\"user\":\"{}\"}}", user.id));
	let response = reqwest::Client::new()
		.post(&cloudflare_url)
		.header(
			"Authorization",
			format!("Bearer {}", CLOUDFLARE_IMAGE_TOKEN.to_string()),
		)
		.multipart(params)
		.send()
		.await;
	if response.is_err() {
		return Err(Status::InternalServerError);
	}
	let response = response.unwrap();
	if !response.status().is_success() {
		return Err(Status::InternalServerError);
	}
	let response = response.json().await;
	if response.is_err() {
		return Err(Status::InternalServerError);
	}
	let response: CloudflareDirectUpload = response.unwrap();
	if !response.success {
		return Err(Status::InternalServerError);
	}
	Ok(Json(response.result.uploadURL))
}

#[post("/upload_archive?<name>", data = "<archive>")]
pub async fn upload_archive(
	archive: Data<'_>,
	name: String,
	user: User,
) -> Result<Json<String>, Status> {
	let stream = archive.open(MAX_FILE_SIZE.mebibytes());
	let bytes = stream.into_bytes().await.unwrap_or(Capped::<Vec<u8>>::new(
		Vec::new(),
		N {
			written: 0,
			complete: true,
		},
	));
	let archive_type = match &bytes[0..4] {
		&[0x50, 0x4B, 0x03, 0x04] => Some("zip"),
		&[0x37, 0x7A, 0xBC, 0xAF] => Some("7z"),
		&[0x52, 0x61, 0x72, 0x21] => Some("rar"),
		_ => None,
	};
	if bytes.len() >= MAX_FILE_SIZE.mebibytes() || bytes.len() == 0 || archive_type.is_none() {
		return Err(Status::BadRequest);
	}

	let result = std::fs::create_dir_all(format!("storage/{}/posts", user.id));
	if result.is_err() {
		return Err(Status::InternalServerError);
	}
	let result = File::create(format!("storage/{}/posts/{}", user.id, name));
	if result.is_err() {
		return Err(Status::InternalServerError);
	}
	let mut file = result.unwrap();
	let result = file.write_all(&bytes);
	if result.is_err() {
		return Err(Status::InternalServerError);
	}
	Ok(Json(format!(
		"{}/storage/{}/posts/{}",
		BASE_URL.to_string(),
		user.id,
		name
	)))
}

#[post("/upload_archive_chunk?<name>&<chunk>", data = "<archive_chunk>")]
pub async fn upload_archive_chunk(
	archive_chunk: Data<'_>,
	name: String,
	chunk: u32,
	user: User,
) -> Status {
	let stream = archive_chunk.open(MAX_FILE_SIZE.mebibytes());
	let bytes = stream.into_bytes().await.unwrap_or(Capped::<Vec<u8>>::new(
		Vec::new(),
		N {
			written: 0,
			complete: false,
		},
	));
	if !bytes.is_complete() {
		return Status::BadRequest;
	}

	let result = std::fs::create_dir_all(format!("storage/{}/posts/{}_chunks", user.id, name));
	if result.is_err() {
		return Status::InternalServerError;
	}
	let result = File::create(format!(
		"storage/{}/posts/{}_chunks/{}",
		user.id, name, chunk
	));
	if result.is_err() {
		return Status::InternalServerError;
	}
	let mut file = result.unwrap();
	let result = file.write_all(&bytes);
	if result.is_err() {
		return Status::InternalServerError;
	}
	Status::Ok
}

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
	let _ = std::fs::remove_dir_all(format!("storage/{}/posts/{}_chunks", user.id, name));
	Ok(Json(format!(
		"{}/storage/{}/posts/{}",
		BASE_URL.to_string(),
		user.id,
		name
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
		.starts_with(format!("{}/cdn-cgi/imagedelivery", BASE_URL.to_string()).as_str())
		|| !post
			.link
			.starts_with(format!("{}/storage/{}/posts/", BASE_URL.to_string(), user.id).as_str())
		|| reqwest::get(post.image.clone()).await.is_err()
		|| reqwest::get(post.link.clone()).await.is_err()
	{
		return Err(Status::BadRequest);
	}
	let post = create_post(
		&mut connection.lock().unwrap(),
		post,
		user,
		update_id.unwrap_or(-1),
	)?;
	Ok(Json(post))
}

#[post("/edit?<update_id>", data = "<post>")]
pub async fn edit(
	connection: &ConnectionState,
	user: User,
	post: Json<PostMetadata>,
	update_id: i32,
) -> Result<Json<Post>, Status> {
	let post = post.into_inner();
	if !owns_post(&mut connection.lock().unwrap(), update_id, user.id) {
		return Err(Status::Unauthorized);
	}

	let result = update_post(&mut connection.lock().unwrap(), post, update_id)?;
	Ok(Json(result))
}

#[get("/<id>")]
pub fn details(connection: &ConnectionState, id: i32) -> Result<Json<DetailedPost>, Status> {
	let result = get_post(&mut connection.lock().unwrap(), id)?;
	Ok(Json(result))
}

#[post("/<id>/like")]
pub fn like(connection: &ConnectionState, id: i32, user: User) -> Result<Json<LikedPost>, Status> {
	let result = like_post_from_ids(&mut connection.lock().unwrap(), user.id, id)?;
	Ok(Json(result))
}
#[post("/<id>/dislike")]
pub fn dislike(
	connection: &ConnectionState,
	id: i32,
	user: User,
) -> Result<Json<DislikedPost>, Status> {
	let result = dislike_post_from_ids(&mut connection.lock().unwrap(), user.id, id)?;
	Ok(Json(result))
}

// Add a dependency to the post with id on dependency
// Return the updated post
#[post("/<id>/dependency/<dependency>")]
pub fn dependency(connection: &ConnectionState, id: i32, dependency: i32, user: User) -> Status {
	let connection = &mut connection.lock().unwrap();
	if !owns_post(connection, id, user.id) {
		Status::Forbidden
	} else {
		add_dependency(connection, id, dependency)
	}
}

#[get("/latest?<name>&<offset>&<game_tag>")]
pub fn latest(
	connection: &ConnectionState,
	name: Option<String>,
	offset: Option<i64>,
	game_tag: Option<i32>,
) -> Result<Json<Vec<DetailedPost>>, Status> {
	let result = get_latest_posts_detailed(
		&mut connection.lock().unwrap(),
		name.unwrap_or(String::new()),
		offset.unwrap_or(0),
		game_tag.unwrap_or(0),
	)?;
	Ok(Json(result))
}

#[get("/popular?<name>&<offset>&<game_tag>")]
pub fn popular(
	connection: &ConnectionState,
	name: Option<String>,
	offset: Option<i64>,
	game_tag: Option<i32>,
) -> Result<Json<Vec<DetailedPost>>, Status> {
	let result = get_popular_posts_detailed(
		&mut connection.lock().unwrap(),
		name.unwrap_or(String::new()),
		offset.unwrap_or(0),
		game_tag.unwrap_or(0),
	)?;
	Ok(Json(result))
}

#[delete("/<id>/delete")]
pub fn delete(connection: &ConnectionState, id: i32, user: User) -> Status {
	delete_post(&mut connection.lock().unwrap(), id, user.id)
}
