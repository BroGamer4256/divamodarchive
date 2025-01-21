use crate::models::*;
use crate::AppState;
use axum::{
	extract::*,
	http::{header, StatusCode},
	response::*,
	routing::*,
	Router,
};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

#[derive(Serialize, Deserialize)]
struct CloudflareDirectUploadResult {
	id: String,
	#[serde(rename = "uploadURL")]
	upload_url: String,
}

#[derive(Serialize, Deserialize)]
struct CloudflareDirectUpload {
	success: bool,
	result: CloudflareDirectUploadResult,
}

pub fn route(state: AppState) -> Router {
	Router::new()
		.route("/api/v1/posts", get(search_posts))
		.route("/api/v1/posts/:id", get(get_post).delete(delete_post))
		.route("/api/v1/posts/edit", post(edit))
		.route("/api/v1/posts/upload_image", get(upload_image))
		.route("/api/v1/posts/upload", get(upload_ws))
		.route("/api/v1/posts/:id/download", get(download))
		.route("/api/v1/posts/:id/like", post(like))
		.route("/api/v1/posts/:id/comment", post(comment))
		.route("/api/v1/posts/:id/author", post(add_author))
		.route("/api/v1/posts/:id/dependency", post(add_dependency))
		.route(
			"/api/v1/posts/:post/comment/:comment",
			delete(delete_comment),
		)
		.with_state(state)
}

async fn upload_image(_: User, State(state): State<AppState>) -> Result<String, StatusCode> {
	let cloudflare_url = format!(
		"https://api.cloudflare.com/client/v4/accounts/{}/images/v2/direct_upload",
		state.config.cloudflare_account_id
	);

	let response = reqwest::Client::new()
		.post(&cloudflare_url)
		.header(
			header::AUTHORIZATION.to_string(),
			format!("Bearer {}", state.config.cloudflare_image_token),
		)
		.send()
		.await;

	let response = match response {
		Ok(response) => response,
		Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
	};
	if !response.status().is_success() {
		return Err(StatusCode::INTERNAL_SERVER_ERROR);
	}
	let response = response.json::<CloudflareDirectUpload>().await;
	let response = match response {
		Ok(response) => response,
		Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
	};
	if response.success {
		Ok(response.result.upload_url)
	} else {
		Err(StatusCode::INTERNAL_SERVER_ERROR)
	}
}

#[derive(Serialize, Deserialize)]
pub struct PostUploadData {
	pub id: Option<i32>,
	pub name: String,
	pub text: String,
	pub post_type: i32,
	pub filename: Option<String>,
	pub image: Option<String>,
	pub images_extra: Option<Vec<String>>,
}

async fn edit(
	user: User,
	State(state): State<AppState>,
	Json(post): Json<PostUploadData>,
) -> Result<(), StatusCode> {
	let Some(post_id) = post.id else {
		return Err(StatusCode::BAD_REQUEST);
	};
	let authors = sqlx::query!(
		"SELECT user_id FROM post_authors WHERE post_id = $1",
		post_id
	)
	.fetch_all(&state.db)
	.await
	.map_err(|_| StatusCode::BAD_REQUEST)?;
	if !authors.iter().any(|u| u.user_id == user.id) {
		return Err(StatusCode::BAD_REQUEST)?;
	}

	sqlx::query!(
		"UPDATE posts SET name = $2, text = $3, type = $4 WHERE id = $1",
		post_id,
		post.name,
		post.text,
		post.post_type
	)
	.execute(&state.db)
	.await
	.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

	if let Some(post) = Post::get_short(post_id, &state.db).await {
		_ = state.meilisearch.add_or_update(&[post], None).await;
	};

	Ok(())
}

async fn upload_ws(ws: ws::WebSocketUpgrade, State(state): State<AppState>) -> Response {
	ws.on_upgrade(move |socket| real_upload_ws(socket, state))
}

async fn real_upload_ws(mut socket: ws::WebSocket, state: AppState) {
	let Some(Ok(message)) = socket.recv().await else {
		return;
	};

	let ws::Message::Text(message) = message else {
		return;
	};

	let Ok(user) = User::parse(&message, &state).await else {
		return;
	};

	let Some(Ok(message)) = socket.recv().await else {
		return;
	};

	let ws::Message::Text(message) = message else {
		return;
	};

	let Ok(params) = serde_json::from_str::<PostUploadData>(&message) else {
		return;
	};

	let Some(filename) = params.filename else {
		return;
	};
	let Some(image) = params.image else {
		return;
	};
	if let Some(post_id) = params.id {
		let Ok(authors) = sqlx::query!(
			"SELECT user_id FROM post_authors WHERE post_id = $1",
			post_id
		)
		.fetch_all(&state.db)
		.await
		else {
			return;
		};
		if !authors.iter().any(|u| u.user_id == user.id) {
			return;
		}
	}

	if !image.starts_with("https://divamodarchive.com/cdn-cgi/imagedelivery")
		|| reqwest::get(&image).await.is_err()
	{
		return;
	}
	if let Some(extra_images) = &params.images_extra {
		for image in extra_images {
			if !image.starts_with("https://divamodarchive.com/cdn-cgi/imagedelivery")
				|| reqwest::get(image).await.is_err()
			{
				return;
			}
		}
	}

	if let Some(id) = params.id {
		let Some(post) = Post::get_full(id, &state.db).await else {
			return;
		};
		if !post.authors.iter().any(|u| u.id == user.id) {
			return;
		}
		_ = tokio::fs::remove_file(format!("pixeldrain/{}", post.file)).await;
	}

	let filepath = format!("{}/{}", user.id, filename);
	_ = tokio::fs::create_dir(format!("/pixeldrain/{}", user.id)).await;
	let Ok(mut file) = tokio::fs::File::create(&format!("/pixeldrain/{}", filepath)).await else {
		return;
	};
	_ = socket.send(ws::Message::Text(String::from("Ready"))).await;

	while let Some(Ok(message)) = socket.recv().await {
		if let ws::Message::Binary(chunk) = message {
			_ = file.write_all(&chunk).await;
			_ = socket.send(ws::Message::Text(String::from("Ready"))).await;
		} else {
			break;
		}
	}

	let mut images = Vec::new();
	images.push(image);
	if let Some(extra_images) = params.images_extra {
		for image in extra_images {
			images.push(image);
		}
	}

	let now = time::OffsetDateTime::now_utc();
	let time = time::PrimitiveDateTime::new(now.date(), now.time());

	let command = tokio::process::Command::new("rclone")
		.arg("link")
		.arg(format!("pixeldrainfs:/divamodarchive/{}", filepath))
		.output()
		.await;
	let Ok(command) = command else {
		return;
	};
	if !command.status.success() {
		return;
	}
	let Ok(path) = String::from_utf8(command.stdout) else {
		return;
	};

	if !path.starts_with("https://pixeldrain.com/d/") {
		return;
	}

	let download = path.trim().replace(
		"https://pixeldrain.com/d/",
		"https://pixeldrain.com/api/filesystem/",
	);
	let download = format!("{download}?download");

	let post_id = if let Some(post_id) = params.id {
		_ = sqlx::query!(
			"UPDATE posts SET name = $2, text = $3, type = $4, file = $5, images = $6, time = $7 WHERE id = $1",
			post_id,
			params.name,
			params.text,
			params.post_type,
			download,
			&images,
			time
		)
		.execute(&state.db)
		.await;

		post_id
	} else {
		let Ok(id) = sqlx::query!("INSERT INTO posts (name, text, images, file, time, type) VALUES ($1, $2, $3, $4, $5, $6) RETURNING ID", params.name, params.text, &images, download, time, params.post_type)
			.fetch_one(&state.db)
			.await else {
				return;
			};

		_ = sqlx::query!(
			"INSERT INTO post_authors (post_id, user_id) VALUES ($1, $2)",
			id.id,
			user.id,
		)
		.execute(&state.db)
		.await;

		id.id
	};

	if let Some(post) = Post::get_short(post_id, &state.db).await {
		_ = state.meilisearch.add_or_update(&[post], None).await;
	};

	_ = socket
		.send(ws::Message::Text(format!("/post/{post_id}")))
		.await;

	_ = socket.close().await;
}

async fn download(
	Path(id): Path<i32>,
	State(state): State<AppState>,
) -> Result<Redirect, StatusCode> {
	let Some(post) = Post::get_short(id, &state.db).await else {
		return Err(StatusCode::NOT_FOUND);
	};

	_ = sqlx::query!(
		"UPDATE posts SET download_count = download_count +1 WHERE id = $1",
		id
	)
	.execute(&state.db)
	.await;

	if let Some(post) = Post::get_short(id, &state.db).await {
		_ = state.meilisearch.add_or_update(&[post], None).await;
	};

	Ok(Redirect::to(&post.file))
}

async fn like(Path(id): Path<i32>, user: User, State(state): State<AppState>) -> StatusCode {
	let Some(post) = Post::get_short(id, &state.db).await else {
		return StatusCode::NOT_FOUND;
	};

	let Ok(has_liked) = sqlx::query!(
		"SELECT COUNT(*) FROM liked_posts WHERE post_id = $1 AND user_id = $2",
		post.id,
		user.id
	)
	.fetch_one(&state.db)
	.await
	else {
		return StatusCode::INTERNAL_SERVER_ERROR;
	};

	if has_liked.count.unwrap_or(0) > 0 {
		_ = sqlx::query!(
			"DELETE FROM liked_posts WHERE post_id = $1 AND user_id = $2",
			post.id,
			user.id
		)
		.execute(&state.db)
		.await;
	} else {
		_ = sqlx::query!(
			"INSERT INTO liked_posts (post_id, user_id) VALUES ($1, $2)",
			post.id,
			user.id
		)
		.execute(&state.db)
		.await;
	}

	if let Some(post) = Post::get_short(id, &state.db).await {
		_ = state.meilisearch.add_or_update(&[post], None).await;
	};

	StatusCode::OK
}

async fn get_post(
	Path(id): Path<i32>,
	State(state): State<AppState>,
) -> Result<Json<Post>, StatusCode> {
	let Some(mut post) = Post::get_full(id, &state.db).await else {
		return Err(StatusCode::NOT_FOUND);
	};
	post.file = format!(
		"https://divamodarchive.com/api/v1/posts/download/{}",
		post.id
	);
	Ok(Json(post))
}

#[derive(Serialize, Deserialize)]
pub struct SearchParams {
	pub query: Option<String>,
	pub sort: Option<Vec<String>>,
	pub filter: Option<String>,
}

async fn search_posts(
	axum_extra::extract::Query(query): axum_extra::extract::Query<SearchParams>,
	State(state): State<AppState>,
) -> Result<Json<Vec<Post>>, String> {
	let mut search = meilisearch_sdk::search::SearchQuery::new(&state.meilisearch);

	search.query = query.query.as_ref().map(|query| query.as_str());
	search.filter = query
		.filter
		.as_ref()
		.map(|filter| meilisearch_sdk::search::Filter::new(sqlx::Either::Left(filter.as_str())));

	let mut sort = vec![];
	if let Some(qsort) = &query.sort {
		for qsort in qsort {
			sort.push(qsort.as_str());
		}
	}
	search.sort = Some(&sort);

	let posts = search
		.with_limit(2048)
		.execute::<Post>()
		.await
		.map_err(|e| e.to_string())?;

	let posts = posts
		.hits
		.into_iter()
		.map(|p| p.result.id)
		.collect::<Vec<_>>();

	let mut vec = Vec::with_capacity(posts.len());
	for id in posts {
		if let Some(mut post) = Post::get_full(id, &state.db).await {
			post.file = format!(
				"https://divamodarchive.com/api/v1/posts/{}/download",
				post.id
			);
			vec.push(post);
		}
	}

	Ok(Json(vec))
}

async fn delete_post(
	Path(id): Path<i32>,
	user: User,
	State(state): State<AppState>,
) -> Result<(), StatusCode> {
	let Some(post) = Post::get_short(id, &state.db).await else {
		return Err(StatusCode::NOT_FOUND);
	};

	if !post.authors.iter().any(|u| u.id == user.id) && !user.is_admin(&state.config) {
		return Err(StatusCode::UNAUTHORIZED);
	}

	_ = sqlx::query!("DELETE FROM posts WHERE id = $1", post.id)
		.execute(&state.db)
		.await;

	Ok(())
}

async fn add_author(
	Path(id): Path<i32>,
	user: User,
	State(state): State<AppState>,
	Json(new_author): Json<String>,
) -> Result<Json<User>, StatusCode> {
	let Some(post) = Post::get_short(id, &state.db).await else {
		return Err(StatusCode::NOT_FOUND);
	};

	if !post.authors.iter().any(|u| u.id == user.id) {
		return Err(StatusCode::UNAUTHORIZED);
	}
	if post.authors.iter().any(|u| u.name == new_author) {
		return Err(StatusCode::BAD_REQUEST);
	}

	let new_author = sqlx::query_as!(User, "SELECT * FROM users WHERE name = $1", new_author)
		.fetch_one(&state.db)
		.await
		.map_err(|_| StatusCode::NOT_FOUND)?;

	_ = sqlx::query!(
		"INSERT INTO post_authors (post_id, user_id) VALUES ($1, $2)",
		post.id,
		new_author.id
	)
	.execute(&state.db)
	.await;

	Ok(Json(new_author))
}

async fn add_dependency(
	Path(id): Path<i32>,
	user: User,
	State(state): State<AppState>,
	Json(dependency): Json<i32>,
) -> Result<Json<Post>, StatusCode> {
	let Some(post) = Post::get_full(id, &state.db).await else {
		return Err(StatusCode::NOT_FOUND);
	};

	let Some(dependency) = Post::get_full(dependency, &state.db).await else {
		return Err(StatusCode::NOT_FOUND);
	};

	if !post.authors.iter().any(|u| u.id == user.id) {
		return Err(StatusCode::UNAUTHORIZED);
	}
	if let Some(dependencies) = post.dependencies {
		if dependencies.iter().any(|p| p.id == dependency.id) {
			return Err(StatusCode::BAD_REQUEST);
		}
	}

	_ = sqlx::query!(
		"INSERT INTO post_dependencies (post_id, dependency_id) VALUES ($1, $2)",
		post.id,
		dependency.id
	)
	.execute(&state.db)
	.await;

	Ok(Json(dependency))
}

#[derive(Serialize, Deserialize)]
struct CommentRequest {
	text: String,
	parent: Option<i32>,
}

async fn comment(
	Path(id): Path<i32>,
	user: User,
	State(state): State<AppState>,
	Json(comment): Json<CommentRequest>,
) -> Result<(), StatusCode> {
	if Post::get_short(id, &state.db).await.is_none() {
		return Err(StatusCode::NOT_FOUND);
	}
	let now = time::OffsetDateTime::now_utc();
	let time = time::PrimitiveDateTime::new(now.date(), now.time());

	_ = sqlx::query!("INSERT INTO post_comments (post_id, user_id, text, parent, time) VALUES ($1, $2, $3, $4, $5)", id, user.id, comment.text, comment.parent, time).execute(&state.db).await;

	Ok(())
}

async fn delete_comment(
	Path((post, comment)): Path<(i32, i32)>,
	user: User,
	State(state): State<AppState>,
) -> Result<(), StatusCode> {
	if Post::get_short(post, &state.db).await.is_none() {
		return Err(StatusCode::NOT_FOUND);
	}

	let comment_user = sqlx::query!(
		"SELECT user_id from post_comments WHERE id = $1 AND post_id = $2",
		comment,
		post
	)
	.fetch_one(&state.db)
	.await
	.map_err(|_| StatusCode::NOT_FOUND)?;

	if user.id != comment_user.user_id && !user.is_admin(&state.config) {
		return Err(StatusCode::UNAUTHORIZED);
	}

	_ = sqlx::query!(
		"DELETE FROM post_comments WHERE id = $1 AND post_id = $2",
		comment,
		post,
	)
	.execute(&state.db)
	.await;

	Ok(())
}
