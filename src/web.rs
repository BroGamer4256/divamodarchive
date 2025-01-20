use crate::models::*;
use crate::{AppState, Config};
use askama::Template;
use axum::{
	extract::*,
	http::{header::*, HeaderMap, StatusCode},
	routing::*,
	Router,
};
use axum_extra::extract::CookieJar;
use meilisearch_sdk::search::*;
use serde::Deserialize;

pub fn route(state: AppState) -> Router {
	Router::new()
		.route("/", get(root))
		.route("/about", get(about))
		.route("/post/:id", get(post_detail))
		.route("/liked/:id", get(liked))
		.route("/user/:id", get(user))
		.route("/upload", get(upload))
		//.route("/search", get(search))
		//.route("/admin", get(admin))
		//.route(/post/:id/edit, get(edit))
		//.route(/post/:id/add_dependency, get(add_dependency))
		//.route(/post/:id/report, get(report))
		.with_state(state)
}

#[derive(Template)]
#[template(path = "root.html")]
pub struct RootTemplate {
	user: Option<User>,
	config: Config,
	posts: Vec<Post>,
}

async fn root(
	user: Option<User>,
	State(state): State<AppState>,
) -> Result<RootTemplate, StatusCode> {
	let latest_posts = sqlx::query!("SELECT id FROM posts ORDER BY time DESC")
		.fetch_all(&state.db)
		.await
		.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
	let mut posts = Vec::new();
	for post in latest_posts {
		if let Some(post) = Post::get_short(post.id, &state.db).await {
			posts.push(post);
		}
	}

	Ok(RootTemplate {
		user,
		config: state.config,
		posts,
	})
}

#[derive(Template)]
#[template(path = "about.html")]
pub struct AboutTemplate {
	user: Option<User>,
	config: Config,
}

async fn about(user: Option<User>, State(state): State<AppState>) -> AboutTemplate {
	AboutTemplate {
		user,
		config: state.config,
	}
}

#[derive(Template)]
#[template(path = "liked.html")]
pub struct LikedTemplate {
	user: Option<User>,
	config: Config,
	posts: Vec<Post>,
	owner: User,
}

async fn liked(
	Path(id): Path<i64>,
	user: Option<User>,
	State(state): State<AppState>,
) -> Result<LikedTemplate, StatusCode> {
	let Some(owner) = User::get(id, &state.db).await else {
		return Err(StatusCode::BAD_REQUEST);
	};

	let liked_posts = sqlx::query!("SELECT post_id FROM liked_posts WHERE user_id = $1", id)
		.fetch_all(&state.db)
		.await
		.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

	let mut posts = Vec::new();
	for post in liked_posts {
		if let Some(post) = Post::get_short(post.post_id, &state.db).await {
			posts.push(post);
		}
	}

	Ok(LikedTemplate {
		user,
		config: state.config,
		posts,
		owner,
	})
}

#[derive(Template)]
#[template(path = "user.html")]
pub struct UserTemplate {
	user: Option<User>,
	config: Config,
	posts: Vec<Post>,
	owner: User,
	total_likes: i64,
	total_downloads: i64,
}

async fn user(
	Path(id): Path<i64>,
	user: Option<User>,
	State(state): State<AppState>,
) -> Result<UserTemplate, StatusCode> {
	let Some(owner) = User::get(id, &state.db).await else {
		return Err(StatusCode::BAD_REQUEST);
	};

	let user_posts = sqlx::query!("SELECT post_id FROM post_authors WHERE user_id = $1", id)
		.fetch_all(&state.db)
		.await
		.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

	let mut posts = Vec::new();
	for post in user_posts {
		if let Some(post) = Post::get_short(post.post_id, &state.db).await {
			posts.push(post);
		}
	}

	let (total_likes, total_downloads) = posts.iter().fold((0, 0), |acc, post| {
		(acc.0 + post.like_count, acc.1 + post.download_count)
	});

	Ok(UserTemplate {
		user,
		config: state.config,
		posts,
		owner,
		total_likes,
		total_downloads,
	})
}

#[derive(Deserialize, Clone)]
struct UploadRequestParams {
	update: i32,
}

#[derive(Template)]
#[template(path = "upload.html")]
pub struct UploadTemplate {
	user: Option<User>,
	config: Config,
	update: Option<Post>,
	jwt: String,
}

async fn upload(
	params: Option<Query<UploadRequestParams>>,
	user: User,
	State(state): State<AppState>,
	cookies: CookieJar,
	headers: HeaderMap,
) -> Result<UploadTemplate, StatusCode> {
	let cookie = cookies.get(&AUTHORIZATION.to_string());
	let token = match cookie {
		Some(cookie) => String::from(cookie.value()),
		None => {
			let auth = headers.get(AUTHORIZATION).ok_or(StatusCode::UNAUTHORIZED)?;
			auth.to_str()
				.map_err(|_| StatusCode::BAD_REQUEST)?
				.replace("Bearer ", "")
		}
	};

	let post = if let Some(params) = params {
		if let Some(post) = Post::get_full(params.update, &state.db).await {
			if post.authors.contains(&user) {
				Some(post)
			} else {
				None
			}
		} else {
			None
		}
	} else {
		None
	};

	Ok(UploadTemplate {
		user: Some(user),
		config: state.config,
		update: post,
		jwt: token,
	})
}

#[derive(Template)]
#[template(path = "post.html")]
pub struct PostTemplate {
	user: Option<User>,
	jwt: Option<String>,
	has_liked: bool,
	is_author: bool,
	config: Config,
	post: Post,
}

async fn post_detail(
	Path(id): Path<i32>,
	user: Option<User>,
	State(state): State<AppState>,
	cookies: CookieJar,
	headers: HeaderMap,
) -> Result<PostTemplate, StatusCode> {
	let Some(post) = Post::get_full(id, &state.db).await else {
		return Err(StatusCode::NOT_FOUND);
	};

	let jwt = if let Some(_) = &user {
		let cookie = cookies.get(&AUTHORIZATION.to_string());
		let token = match cookie {
			Some(cookie) => String::from(cookie.value()),
			None => {
				let auth = headers.get(AUTHORIZATION).ok_or(StatusCode::UNAUTHORIZED)?;
				auth.to_str()
					.map_err(|_| StatusCode::BAD_REQUEST)?
					.replace("Bearer ", "")
			}
		};
		Some(token)
	} else {
		None
	};

	let has_liked = if let Some(user) = &user {
		let Ok(has_liked) = sqlx::query!(
			"SELECT COUNT(*) FROM liked_posts WHERE post_id = $1 AND user_id = $2",
			post.id,
			user.id
		)
		.fetch_one(&state.db)
		.await
		else {
			return Err(StatusCode::INTERNAL_SERVER_ERROR);
		};

		has_liked.count.unwrap_or(0) > 0
	} else {
		false
	};

	let is_author = if let Some(user) = &user {
		post.authors.iter().any(|u| u.id == user.id)
	} else {
		false
	};

	Ok(PostTemplate {
		user,
		jwt,
		has_liked,
		is_author,
		config: state.config,
		post,
	})
}

async fn search(
	user: Option<User>,
	State(state): State<AppState>,
) -> Result<RootTemplate, StatusCode> {
	let posts = SearchQuery::new(&state.meilisearch)
		.with_query("coolchar")
		.with_sort(&["download_count"])
		.with_limit(2048)
		.execute()
		.await
		.map_err(|_| StatusCode::NOT_FOUND)?
		.hits
		.into_iter()
		.map(|p| p.result)
		.collect();

	Ok(RootTemplate {
		user,
		config: state.config,
		posts,
	})
}
