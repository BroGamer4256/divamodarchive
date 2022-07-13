use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Write;

use crate::models::*;
use crate::posts::*;
use rocket::data::{Capped, Data, ToByteUnit, N};
use rocket::http::Status;
use rocket::serde::json::Json;

#[post("/upload_image", data = "<image>")]
pub async fn upload_image(image: Data<'_>, _verified: Verified) -> Result<Json<String>, Status> {
	let stream = image.open(MAX_IMAGE_SIZE.mebibytes());
	let bytes = stream.into_bytes().await.unwrap_or(Capped::<Vec<u8>>::new(
		Vec::new(),
		N {
			written: 0,
			complete: true,
		},
	));

	let image_type = match &bytes[0..4] {
		&[0x89, 0x50, 0x4e, 0x47] => Some("png"),
		_ => None,
	};
	if bytes.len() >= MAX_IMAGE_SIZE.mebibytes() || bytes.len() == 0 || image_type.is_none() {
		return Err(Status::BadRequest);
	}
	let image_type = image_type.unwrap();
	let mut hasher = DefaultHasher::new();
	bytes.hash(&mut hasher);
	let hash = hasher.finish();

	let result = File::create(format!("storage/images/{:x}.{}", hash, image_type));

	if result.is_err() {
		return Err(Status::InternalServerError);
	}
	let mut file = result.unwrap();
	let result = file.write_all(&bytes);
	if result.is_err() {
		return Err(Status::InternalServerError);
	}

	Ok(Json(format!(
		"{}/storage/images/{:x}.{}",
		BASE_URL.to_string(),
		hash,
		image_type
	)))
}

#[post("/upload_archive", data = "<archive>")]
pub async fn upload_archive(
	archive: Data<'_>,
	_verified: Verified,
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
	let archive_type = archive_type.unwrap();
	let mut hasher = DefaultHasher::new();
	bytes.hash(&mut hasher);
	let hash = hasher.finish();

	let result = File::create(format!("storage/posts/{:x}.{}", hash, archive_type));

	if result.is_err() {
		return Err(Status::InternalServerError);
	}
	let mut file = result.unwrap();
	let result = file.write_all(&bytes);
	if result.is_err() {
		return Err(Status::InternalServerError);
	}
	Ok(Json(format!(
		"{}/storage/posts/{:x}.{}",
		BASE_URL.to_string(),
		hash,
		archive_type
	)))
}

#[post("/upload", data = "<post>")]
pub async fn upload(
	connection: &ConnectionState,
	user: User,
	post: Json<PostUnidentified>,
) -> Result<Json<Post>, Status> {
	let post = post.into_inner();
	if !post
		.image
		.starts_with(format!("{}/storage/images/", BASE_URL.to_string()).as_str())
		|| !post
			.link
			.starts_with(format!("{}/storage/posts/", BASE_URL.to_string()).as_str())
		|| reqwest::get(post.image.clone()).await.is_err()
		|| reqwest::get(post.link.clone()).await.is_err()
	{
		return Err(Status::BadRequest);
	}
	let post = create_post(&mut connection.lock().unwrap(), post, user)?;
	Ok(Json(post))
}

#[get("/<id>")]
pub fn details(connection: &ConnectionState, id: i32) -> Result<Json<PostWithUser>, Status> {
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

#[get("/latest?<name>&<offset>")]
pub fn latest(
	connection: &ConnectionState,
	name: Option<String>,
	offset: Option<i64>,
) -> Result<Json<Vec<DetailedPost>>, Status> {
	let result = get_latest_posts_detailed(
		&mut connection.lock().unwrap(),
		name.unwrap_or(String::new()),
		offset.unwrap_or(0),
	)?;
	Ok(Json(result))
}

#[get("/popular?<name>&<offset>")]
pub fn popular(
	connection: &ConnectionState,
	name: Option<String>,
	offset: Option<i64>,
) -> Result<Json<Vec<DetailedPost>>, Status> {
	let result = get_popular_posts_detailed(
		&mut connection.lock().unwrap(),
		name.unwrap_or(String::new()),
		offset.unwrap_or(0),
	)?;
	Ok(Json(result))
}

#[delete("/<id>/delete")]
pub fn delete(connection: &ConnectionState, id: i32, user: User) -> Status {
	delete_post(&mut connection.lock().unwrap(), id, user.id)
}
