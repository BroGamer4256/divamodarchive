use crate::api::ids::*;
use crate::models::*;
use crate::{AppState, Config};
use askama::Template;
use axum::{
	extract::*,
	http::{header::*, StatusCode},
	response::*,
	routing::*,
	RequestPartsExt, Router,
};
use axum_extra::extract::CookieJar;
use std::collections::*;

pub fn route(state: AppState) -> Router {
	Router::new()
		.route("/", get(search))
		.route("/about", get(about))
		.route("/post/:id", get(post_detail))
		.route("/posts/:id", get(post_redirect))
		.route("/post/:id/edit", get(upload))
		.route("/post/:id/report", get(report))
		.route("/liked/:id", get(liked))
		.route("/user/:id", get(user))
		.route("/upload", get(upload))
		.route("/settings", get(settings))
		.route("/pvs", get(pvs))
		.route("/modules", get(modules))
		.route("/cstm_items", get(cstm_items))
		//.route("/admin", get(admin))
		.with_state(state)
}

mod filters {
	pub fn prettify_num<T: std::fmt::Display>(s: T) -> askama::Result<String> {
		let num: u64 = match s.to_string().parse() {
			Ok(num) => num,
			Err(e) => return Err(askama::Error::Custom(Box::new(e))),
		};

		let suffixes = ["", "K", "M", "B"];
		let mut remainder = 0;
		let mut value = num;
		for suffix in suffixes {
			if value < 1000 {
				if remainder > 0 {
					return Ok(format!("{value}.{remainder}{suffix}"));
				} else {
					return Ok(format!("{value}{suffix}"));
				}
			}
			remainder = (value % 1000) / 100;
			value /= 1000;
		}

		Ok(String::new())
	}
}

#[derive(Clone)]
pub struct BaseTemplate {
	pub user: Option<User>,
	pub config: Config,
	pub jwt: Option<String>,
	pub report_count: Option<i64>,
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for BaseTemplate
where
	S: Send + Sync,
	AppState: FromRef<S>,
{
	type Rejection = std::convert::Infallible;

	async fn from_request_parts(
		parts: &mut axum::http::request::Parts,
		state: &S,
	) -> Result<Self, Self::Rejection> {
		let cookies = parts.extract::<CookieJar>().await.unwrap();
		let jwt = if let Some(cookie) = cookies.get(&AUTHORIZATION.to_string()) {
			Some(String::from(cookie.value()))
		} else {
			if let Some(auth) = parts.headers.get(AUTHORIZATION) {
				if let Ok(auth) = auth.to_str() {
					Some(String::from(auth.replace("Bearer ", "")))
				} else {
					None
				}
			} else {
				None
			}
		};

		let user = if jwt.is_some() {
			User::from_request_parts(parts, state).await.ok()
		} else {
			None
		};
		let state: AppState = AppState::from_ref(state);

		let report_count = if let Some(user) = &user {
			if user.is_admin(&state.config) {
				sqlx::query!("SELECT COUNT(*) FROM reports WHERE admin_handled IS NULL")
					.fetch_one(&state.db)
					.await
					.ok()
					.map(|count| count.count.unwrap_or(0))
			} else {
				None
			}
		} else {
			None
		};

		Ok(Self {
			user,
			config: state.config,
			jwt,
			report_count,
		})
	}
}

impl BaseTemplate {
	pub fn theme(&self) -> Theme {
		let Some(user) = &self.user else {
			return Theme::default();
		};
		user.theme
	}
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
) -> Result<LikedTemplate, Result<UnauthorizedTemplate, StatusCode>> {
	let Some(owner) = User::get(id, &state.db).await else {
		return Err(Err(StatusCode::BAD_REQUEST));
	};

	if !owner.public_likes && !base.user.as_ref().map_or(false, |user| user.id == owner.id) {
		return Err(Ok(UnauthorizedTemplate { base }));
	}

	let liked_posts = sqlx::query!(
		r#"
		SELECT p.id
		FROM liked_posts lp
		LEFT JOIN posts p ON lp.post_id = p.id
		WHERE lp.user_id = $1
		ORDER by p.time DESC
		"#,
		id,
	)
	.fetch_all(&state.db)
	.await
	.map_err(|_| Err(StatusCode::INTERNAL_SERVER_ERROR))?;

	let mut posts = Vec::new();
	for post in liked_posts {
		if let Some(post) = Post::get_short(post.id, &state.db).await {
			posts.push(post);
		}
	}

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
		SELECT p.id
		FROM post_authors pa
		LEFT JOIN posts p ON pa.post_id = p.id
		WHERE pa.user_id = $1
		ORDER BY p.time DESC
		"#,
		id
	)
	.fetch_all(&state.db)
	.await
	.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

	let mut posts = Vec::new();
	for post in user_posts {
		if let Some(post) = Post::get_short(post.id, &state.db).await {
			posts.push(post);
		}
	}

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
	base: BaseTemplate,
	update_id: Option<Path<i32>>,
	user: User,
	State(state): State<AppState>,
) -> Result<UploadTemplate, StatusCode> {
	let Some(jwt) = base.jwt.clone() else {
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
	pvs: PvSearch,
	modules: ModuleSearch,
	cstm_items: CstmItemSearch,
	pv_easy_count: usize,
	pv_normal_count: usize,
	pv_hard_count: usize,
	pv_extreme_count: usize,
	pv_exextreme_count: usize,
}

async fn post_redirect(Path(id): Path<i32>) -> Redirect {
	Redirect::permanent(&format!("https://divamodarchive.com/post/{id}"))
}

async fn post_detail(
	Path(id): Path<i32>,
	user: Option<User>,
	State(state): State<AppState>,
	base: BaseTemplate,
) -> Result<PostTemplate, Result<UnauthorizedTemplate, StatusCode>> {
	let Some(post) = Post::get_full(id, &state.db).await else {
		return Err(Err(StatusCode::NOT_FOUND));
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
			return Err(Err(StatusCode::INTERNAL_SERVER_ERROR));
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

	let Json(pvs) = search_pvs(
		axum_extra::extract::Query(SearchParams {
			query: None,
			filter: Some(format!("post={}", post.id)),
			limit: Some(2000),
			offset: Some(0),
		}),
		State(state.clone()),
	)
	.await
	.unwrap_or_default();

	let Json(modules) = crate::api::ids::search_modules(
		axum_extra::extract::Query(SearchParams {
			query: None,
			filter: Some(format!("post_id={}", post.id)),
			limit: Some(2000),
			offset: Some(0),
		}),
		State(state.clone()),
	)
	.await
	.unwrap_or_default();

	let Json(cstm_items) = crate::api::ids::search_cstm_items(
		axum_extra::extract::Query(SearchParams {
			query: None,
			filter: Some(format!("post_id={}", post.id)),
			limit: Some(2000),
			offset: Some(0),
		}),
		State(state.clone()),
	)
	.await
	.unwrap_or_default();

	let pv_easy_count = pvs.pvs.iter().filter(|pv| pv.levels[0].is_some()).count();
	let pv_normal_count = pvs.pvs.iter().filter(|pv| pv.levels[1].is_some()).count();
	let pv_hard_count = pvs.pvs.iter().filter(|pv| pv.levels[2].is_some()).count();
	let pv_extreme_count = pvs.pvs.iter().filter(|pv| pv.levels[3].is_some()).count();
	let pv_exextreme_count = pvs.pvs.iter().filter(|pv| pv.levels[4].is_some()).count();

	Ok(PostTemplate {
		user,
		jwt: base.jwt.clone(),
		has_liked,
		is_author,
		base,
		post,
		config: state.config,
		pvs,
		modules,
		cstm_items,
		pv_easy_count,
		pv_normal_count,
		pv_hard_count,
		pv_extreme_count,
		pv_exextreme_count,
	})
}

#[derive(Template)]
#[template(path = "search.html")]
struct SearchTemplate {
	base: BaseTemplate,
	posts: Vec<Post>,
}

async fn search(
	base: BaseTemplate,
	State(state): State<AppState>,
) -> Result<SearchTemplate, StatusCode> {
	let latest_posts = sqlx::query!(
		r#"
		SELECT p.id, p.name, p.text, p.images, p.files, p.time, p.type as post_type, p.download_count, p.local_files, like_count.like_count
		FROM posts p
		LEFT JOIN (SELECT post_id, COUNT(*) as like_count FROM liked_posts GROUP BY post_id) AS like_count ON p.id = like_count.post_id
		ORDER BY p.time DESC
		LIMIT 20
		"#
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
			local_files: post.local_files,
		})
		.collect();

	Ok(SearchTemplate { base, posts })
}

#[derive(Template)]
#[template(path = "settings.html")]
struct SettingsTemplate {
	base: BaseTemplate,
	user: User,
}

async fn settings(base: BaseTemplate, user: User) -> SettingsTemplate {
	SettingsTemplate { base, user }
}

#[derive(Template)]
#[template(path = "report.html")]
struct ReportTemplate {
	base: BaseTemplate,
	post: Post,
}

async fn report(
	Path(id): Path<i32>,
	base: BaseTemplate,
	_: User,
	State(state): State<AppState>,
) -> Result<ReportTemplate, StatusCode> {
	let Some(post) = Post::get_short(id, &state.db).await else {
		return Err(StatusCode::NOT_FOUND);
	};

	Ok(ReportTemplate { base, post })
}

#[derive(Template)]
#[template(path = "pvs.html")]
struct PvsTemplate {
	base: BaseTemplate,
	pvs: PvSearch,
}

async fn pvs(base: BaseTemplate, State(state): State<AppState>) -> Result<PvsTemplate, StatusCode> {
	let Json(pvs) = crate::api::ids::search_pvs(
		axum_extra::extract::Query(SearchParams {
			query: None,
			filter: None,
			limit: Some(20),
			offset: Some(0),
		}),
		State(state),
	)
	.await
	.unwrap_or_default();

	return Ok(PvsTemplate { base, pvs });
}

#[derive(Template)]
#[template(path = "modules.html")]
struct ModulesTemplate {
	base: BaseTemplate,
	modules: ModuleSearch,
}

async fn modules(
	base: BaseTemplate,
	State(state): State<AppState>,
) -> Result<ModulesTemplate, StatusCode> {
	let Json(modules) = crate::api::ids::search_modules(
		axum_extra::extract::Query(SearchParams {
			query: None,
			filter: None,
			limit: Some(20),
			offset: Some(0),
		}),
		State(state),
	)
	.await
	.unwrap_or_default();

	return Ok(ModulesTemplate { base, modules });
}

#[derive(Template)]
#[template(path = "cstm_items.html")]
struct CstmItemsTemplate {
	base: BaseTemplate,
	cstm_items: CstmItemSearch,
}

async fn cstm_items(
	base: BaseTemplate,
	State(state): State<AppState>,
) -> Result<CstmItemsTemplate, StatusCode> {
	let Json(cstm_items) = crate::api::ids::search_cstm_items(
		axum_extra::extract::Query(SearchParams {
			query: None,
			filter: None,
			limit: Some(20),
			offset: Some(0),
		}),
		State(state),
	)
	.await
	.unwrap_or_default();

	return Ok(CstmItemsTemplate { base, cstm_items });
}
