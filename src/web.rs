use crate::models::*;
use crate::{AppState, Config};
use askama::Template;
use axum::{
	extract::*,
	http::{header::*, StatusCode},
	routing::*,
	RequestPartsExt, Router,
};
use axum_extra::extract::CookieJar;

pub fn route(state: AppState) -> Router {
	Router::new()
		.route("/", get(root))
		.route("/about", get(about))
		.route("/post/:id", get(post_detail))
		.route("/post/:id/edit", get(upload))
		.route("/liked/:id", get(liked))
		.route("/user/:id", get(user))
		.route("/upload", get(upload))
		.route("/search", get(search))
		//.route("/admin", get(admin))
		//.route("/post/:id/report", get(report))
		.with_state(state)
}

pub struct BaseTemplate {
	user: Option<User>,
	config: Config,
	jwt: Option<String>,
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for BaseTemplate
where
	S: Send + Sync,
	AppState: FromRef<S>,
{
	type Rejection = StatusCode;

	async fn from_request_parts(
		parts: &mut axum::http::request::Parts,
		state: &S,
	) -> Result<Self, Self::Rejection> {
		let cookies = parts
			.extract::<CookieJar>()
			.await
			.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
		let cookie = cookies.get(&AUTHORIZATION.to_string());
		let jwt = match cookie {
			Some(cookie) => Some(String::from(cookie.value())),
			None => {
				if let Some(auth) = parts.headers.get(AUTHORIZATION) {
					if let Ok(auth) = auth.to_str() {
						Some(String::from(auth.replace("Bearer ", "")))
					} else {
						None
					}
				} else {
					None
				}
			}
		};

		let user = User::from_request_parts(parts, state).await.ok();
		let state: AppState = AppState::from_ref(state);

		Ok(Self {
			user,
			config: state.config,
			jwt,
		})
	}
}

impl BaseTemplate {
	fn show_explicit(&self) -> bool {
		let Some(user) = &self.user else { return false };
		user.show_explicit
	}
}

#[derive(Template)]
#[template(path = "root.html")]
struct RootTemplate {
	base: BaseTemplate,
	posts: Vec<Post>,
}

async fn root(
	base: BaseTemplate,
	State(state): State<AppState>,
) -> Result<RootTemplate, StatusCode> {
	let latest_posts = sqlx::query!(
		r#"
		SELECT p.id, p.name, p.text, p.images, p.files, p.time, p.type as post_type, p.download_count, p.explicit, p.local_files, like_count.like_count
		FROM posts p
		LEFT JOIN (SELECT post_id, COUNT(*) as like_count FROM liked_posts GROUP BY post_id) AS like_count ON p.id = like_count.post_id
		WHERE explicit = $1 OR explicit = false
		ORDER BY time DESC
		LIMIT 40
		"#,
		base.show_explicit()
	)
	.fetch_all(&state.db)
	.await
	.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

	let posts = latest_posts
		.into_iter()
		.map(|post| Post {
			id: post.id,
			name: post.name,
			text: post.text,
			images: post.images,
			files: post.files,
			time: post.time.assume_offset(time::UtcOffset::UTC),
			post_type: post.post_type.into(),
			download_count: post.download_count,
			like_count: post.like_count.unwrap_or(0),
			authors: vec![],
			dependencies: None,
			comments: None,
			explicit: post.explicit,
			local_files: post.local_files,
		})
		.collect();

	Ok(RootTemplate { posts, base })
}

#[derive(Template)]
#[template(path = "about.html")]
struct AboutTemplate {
	base: BaseTemplate,
}

async fn about(base: BaseTemplate) -> AboutTemplate {
	AboutTemplate { base }
}

#[derive(Template)]
#[template(path = "liked.html")]
struct LikedTemplate {
	base: BaseTemplate,
	posts: Vec<Post>,
	owner: User,
}

async fn liked(
	Path(id): Path<i64>,
	base: BaseTemplate,
	State(state): State<AppState>,
) -> Result<LikedTemplate, StatusCode> {
	let Some(owner) = User::get(id, &state.db).await else {
		return Err(StatusCode::BAD_REQUEST);
	};

	if !owner.public_likes && !base.user.as_ref().map_or(false, |user| user.id == owner.id) {
		return Err(StatusCode::UNAUTHORIZED);
	}

	let liked_posts = sqlx::query!(
		r#"
		SELECT p.id, p.name, p.text, p.images, p.files, p.time, p.type as post_type, p.download_count, p.explicit, p.local_files, like_count.like_count
		FROM liked_posts lp
		LEFT JOIN posts p ON lp.post_id = p.id
		LEFT JOIN (SELECT post_id, COUNT(*) as like_count FROM liked_posts GROUP BY post_id) AS like_count ON p.id = like_count.post_id
		WHERE lp.user_id = $1
		AND (p.explicit = $2 OR p.explicit = false)
		"#,
		id,
		base.show_explicit(),
	)
	.fetch_all(&state.db)
	.await
	.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

	let posts = liked_posts
		.into_iter()
		.map(|post| Post {
			id: post.id,
			name: post.name,
			text: post.text,
			images: post.images,
			files: post.files,
			time: post.time.assume_offset(time::UtcOffset::UTC),
			post_type: post.post_type.into(),
			download_count: post.download_count,
			like_count: post.like_count.unwrap_or(0),
			authors: vec![],
			dependencies: None,
			comments: None,
			explicit: post.explicit,
			local_files: post.local_files,
		})
		.collect();

	Ok(LikedTemplate { base, posts, owner })
}

#[derive(Template)]
#[template(path = "user.html")]
struct UserTemplate {
	base: BaseTemplate,
	posts: Vec<Post>,
	owner: User,
	total_likes: i64,
	total_downloads: i64,
}

async fn user(
	Path(id): Path<i64>,
	base: BaseTemplate,
	State(state): State<AppState>,
) -> Result<UserTemplate, StatusCode> {
	let Some(owner) = User::get(id, &state.db).await else {
		return Err(StatusCode::BAD_REQUEST);
	};

	let user_posts = sqlx::query!(
		r#"
		SELECT p.id, p.name, p.text, p.images, p.files, p.time, p.type as post_type, p.download_count, p.explicit, p.local_files, like_count.like_count
		FROM post_authors pa
		LEFT JOIN posts p ON pa.post_id = p.id
		LEFT JOIN (SELECT post_id, COUNT(*) as like_count FROM liked_posts GROUP BY post_id) AS like_count ON p.id = like_count.post_id
		WHERE pa.user_id = $1
		AND (p.explicit = $2 OR p.explicit = false)
		"#,
		id,
		base.show_explicit()
	)
	.fetch_all(&state.db)
	.await
	.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

	let posts: Vec<Post> = user_posts
		.into_iter()
		.map(|post| Post {
			id: post.id,
			name: post.name,
			text: post.text,
			images: post.images,
			files: post.files,
			time: post.time.assume_offset(time::UtcOffset::UTC),
			post_type: post.post_type.into(),
			download_count: post.download_count,
			like_count: post.like_count.unwrap_or(0),
			authors: vec![],
			dependencies: None,
			comments: None,
			explicit: post.explicit,
			local_files: post.local_files,
		})
		.collect();

	let (total_likes, total_downloads) = posts.iter().fold((0, 0), |acc, post| {
		(acc.0 + post.like_count, acc.1 + post.download_count)
	});

	Ok(UserTemplate {
		base,
		posts,
		owner,
		total_likes,
		total_downloads,
	})
}

#[derive(Template)]
#[template(path = "upload.html")]
struct UploadTemplate {
	base: BaseTemplate,
	update: Option<Post>,
	jwt: String,
	user: User,
}

async fn upload(
	update_id: Option<Path<i32>>,
	State(state): State<AppState>,
	base: BaseTemplate,
) -> Result<UploadTemplate, StatusCode> {
	let Some(jwt) = base.jwt.clone() else {
		return Err(StatusCode::UNAUTHORIZED);
	};
	let Some(user) = base.user.clone() else {
		return Err(StatusCode::UNAUTHORIZED);
	};

	let post = if let Some(Path(id)) = update_id {
		if let Some(post) = Post::get_full(id, &state.db).await {
			if post.authors.contains(&user) {
				Some(post)
			} else {
				return Err(StatusCode::UNAUTHORIZED);
			}
		} else {
			return Err(StatusCode::UNAUTHORIZED);
		}
	} else {
		None
	};

	Ok(UploadTemplate {
		base,
		update: post,
		jwt,
		user,
	})
}

#[derive(Template)]
#[template(path = "post.html")]
struct PostTemplate {
	base: BaseTemplate,
	user: Option<User>,
	jwt: Option<String>,
	has_liked: bool,
	is_author: bool,
	post: Post,
	config: Config,
}

async fn post_detail(
	Path(id): Path<i32>,
	user: Option<User>,
	State(state): State<AppState>,
	base: BaseTemplate,
) -> Result<PostTemplate, StatusCode> {
	let Some(post) = Post::get_full(id, &state.db).await else {
		return Err(StatusCode::NOT_FOUND);
	};

	if post.explicit && !base.show_explicit() {
		return Err(StatusCode::UNAUTHORIZED);
	}

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
		jwt: base.jwt.clone(),
		has_liked,
		is_author,
		base,
		post,
		config: state.config,
	})
}

#[derive(Template)]
#[template(path = "search.html")]
struct SearchTemplate {
	base: BaseTemplate,
}

async fn search(base: BaseTemplate) -> Result<SearchTemplate, StatusCode> {
	Ok(SearchTemplate { base })
}
