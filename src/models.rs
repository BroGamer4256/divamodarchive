use crate::{AppState, Config};
use axum::extract::*;
use axum::http::{header::*, request::*, StatusCode};
use axum::response::*;
use axum::RequestPartsExt;
use axum_extra::extract::cookie::{Cookie, CookieJar};
use jsonwebtoken::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct User {
	pub id: i64,
	pub name: String,
	pub avatar: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Token {
	pub exp: i64,
	pub user_id: i64,
}

#[repr(i32)]
#[derive(PartialEq, Serialize, Deserialize)]
pub enum PostType {
	Plugin = 0,
	Module = 1,
	Song = 2,
	Ui = 3,
	Other = 4,
}

impl From<i32> for PostType {
	fn from(value: i32) -> Self {
		match value {
			0 => Self::Plugin,
			1 => Self::Module,
			2 => Self::Song,
			3 => Self::Ui,
			_ => Self::Other,
		}
	}
}

impl std::fmt::Display for PostType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(match self {
			PostType::Plugin => "Plugin",
			PostType::Module => "Module",
			PostType::Song => "Song",
			PostType::Ui => "UI",
			PostType::Other => "Other",
		})
	}
}

#[derive(Serialize, Deserialize)]
pub struct Post {
	pub id: i32,
	pub name: String,
	pub text: String,
	pub images: Vec<String>,
	pub file: String,
	pub time: time::PrimitiveDateTime,
	pub post_type: PostType,
	pub download_count: i64,
	pub like_count: i64,
	pub authors: Vec<User>,
	pub dependencies: Option<Vec<Post>>,
	#[serde(skip)]
	pub comments: Option<slab_tree::Tree<Comment>>,
}

pub struct Comment {
	pub id: i32,
	pub user: User,
	pub text: String,
	pub time: time::PrimitiveDateTime,
}

impl PartialEq for Comment {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id
	}
}

impl Post {
	pub async fn get_full(id: i32, db: &sqlx::Pool<sqlx::Postgres>) -> Option<Self> {
		let post = sqlx::query!(
			r#"
			SELECT p.id, p.name, p.text, p.images, p.file, p.time, p.type as post_type, p.download_count, like_count.like_count
			FROM posts p
			LEFT JOIN post_comments c ON p.id = c.post_id
			LEFT JOIN (SELECT post_id, COUNT(*) as like_count FROM liked_posts GROUP BY post_id) AS like_count ON p.id = like_count.post_id
			WHERE p.id = $1
			"#,
			id
		)
		.fetch_one(db)
		.await
		.ok()?;

		let authors = sqlx::query_as!(
			User,
			r#"
			SELECT u.id, u.name, u.avatar
			FROM post_authors pa
			JOIN users u ON pa.user_id = u.id
			WHERE pa.post_id = $1
			"#,
			id
		)
		.fetch_all(db)
		.await
		.ok()?;

		let dependencies = sqlx::query!(
			r#"
			SELECT p.id, p.name, p.text, p.images, p.file, p.time, p.type as post_type, p.download_count, COALESCE(like_count.count, 0) AS "like_count!"
			FROM post_dependencies pd
			LEFT JOIN posts p ON pd.dependency_id = p.id
			LEFT JOIN (SELECT post_id, COUNT(*) as count FROM liked_posts GROUP BY post_id) AS like_count ON p.id = like_count.post_id
			LEFT JOIN post_authors pa ON pa.post_id = p.id
			LEFT JOIN users u ON pa.user_id = u.id
			WHERE pd.post_id = $1
			"#,
			id
		)
		.fetch_all(db)
		.await
		.ok()?;

		let mut deps = vec![];
		for dep in dependencies {
			let Ok(authors) = sqlx::query_as!(
				User,
				r#"
				SELECT u.id, u.name, u.avatar
				FROM post_authors pa
				LEFT JOIN users u ON pa.user_id = u.id
				WHERE pa.post_id = $1
				"#,
				dep.id
			)
			.fetch_all(db)
			.await
			else {
				continue;
			};

			deps.push(Post {
				id: dep.id,
				name: dep.name,
				text: dep.text,
				images: dep.images,
				file: dep.file,
				time: dep.time,
				post_type: dep.post_type.into(),
				download_count: dep.download_count,
				like_count: dep.like_count,
				authors,
				dependencies: None,
				comments: None,
			});
		}

		let comments = sqlx::query!(
			r#"
			SELECT c.id, c.text, c.parent, c.time, u.id as user_id, u.name as user_name, u.avatar as user_avatar
			FROM post_comments c
			LEFT JOIN users u ON c.user_id = u.id
			WHERE c.post_id = $1
			ORDER BY c.time ASC
			"#,
			id
		)
		.fetch_all(db)
		.await
		.ok()?;

		let mut tree = slab_tree::TreeBuilder::new()
			.with_root(Comment {
				id: -1,
				user: User {
					id: -1,
					name: String::new(),
					avatar: String::new(),
				},
				text: String::new(),
				time: time::PrimitiveDateTime::MIN,
			})
			.build();
		let root = tree.root_id()?;
		let mut ids = BTreeMap::new();

		for comment in comments {
			if let Some(parent_id) = comment.parent {
				if let Some(parent_node) = ids.get(&parent_id) {
					let mut node = tree.get_mut(*parent_node)?;
					let node_id = node
						.append(Comment {
							id: comment.id,
							user: User {
								id: comment.user_id,
								name: comment.user_name.clone(),
								avatar: comment.user_avatar.clone(),
							},
							text: comment.text.clone(),
							time: comment.time,
						})
						.node_id();
					ids.insert(comment.id, node_id);
				}
			} else {
				let mut root = tree.get_mut(root)?;
				let node_id = root
					.append(Comment {
						id: comment.id,
						user: User {
							id: comment.user_id,
							name: comment.user_name.clone(),
							avatar: comment.user_avatar.clone(),
						},
						text: comment.text.clone(),
						time: comment.time,
					})
					.node_id();
				ids.insert(comment.id, node_id);
			}
		}

		Some(Post {
			id,
			name: post.name,
			text: post.text,
			images: post.images,
			file: post.file,
			time: post.time,
			post_type: post.post_type.into(),
			download_count: post.download_count,
			like_count: post.like_count.unwrap_or(0),
			authors,
			dependencies: Some(deps),
			comments: Some(tree),
		})
	}

	pub async fn get_short(id: i32, db: &sqlx::Pool<sqlx::Postgres>) -> Option<Self> {
		let post = sqlx::query!(
			r#"
			SELECT p.id, p.name, p.text, p.images, p.file, p.time, p.type as post_type, p.download_count, like_count.like_count
			FROM posts p
			LEFT JOIN post_comments c ON p.id = c.post_id
			LEFT JOIN (SELECT post_id, COUNT(*) as like_count FROM liked_posts GROUP BY post_id) AS like_count ON p.id = like_count.post_id
			WHERE p.id = $1
			"#,
			id
		)
		.fetch_one(db)
		.await
		.ok()?;

		let authors = sqlx::query_as!(
			User,
			r#"
			SELECT u.id, u.name, u.avatar
			FROM post_authors pa
			LEFT JOIN users u ON pa.user_id = u.id
			WHERE pa.post_id = $1
			"#,
			id
		)
		.fetch_all(db)
		.await
		.ok()?;

		Some(Post {
			id,
			name: post.name,
			text: post.text,
			images: post.images,
			file: post.file,
			time: post.time,
			post_type: post.post_type.into(),
			download_count: post.download_count,
			like_count: post.like_count.unwrap_or(0),
			authors,
			dependencies: None,
			comments: None,
		})
	}
}

impl User {
	pub fn is_admin(&self, config: &Config) -> bool {
		config.admins.contains(&self.id)
	}

	pub async fn parse(token: &str, state: &AppState) -> Result<Self, StatusCode> {
		let token_data = jsonwebtoken::decode::<Token>(
			&token,
			&state.config.decoding_key,
			&Validation::default(),
		)
		.map_err(|_| StatusCode::UNAUTHORIZED)?;

		sqlx::query_as!(
			User,
			"SELECT * FROM users WHERE id = $1",
			token_data.claims.user_id
		)
		.fetch_one(&state.db)
		.await
		.map_err(|_| StatusCode::UNAUTHORIZED)
	}

	pub async fn get(id: i64, db: &sqlx::Pool<sqlx::Postgres>) -> Option<Self> {
		sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id)
			.fetch_one(db)
			.await
			.ok()
	}
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for User
where
	S: Send + Sync,
	AppState: FromRef<S>,
{
	type Rejection = StatusCode;

	async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
		let cookies = parts
			.extract::<CookieJar>()
			.await
			.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
		let cookie = cookies.get(&AUTHORIZATION.to_string());
		let token = match cookie {
			Some(cookie) => String::from(cookie.value()),
			None => {
				let auth = parts
					.headers
					.get(AUTHORIZATION)
					.ok_or(StatusCode::UNAUTHORIZED)?;
				auth.to_str()
					.map_err(|_| StatusCode::BAD_REQUEST)?
					.replace("Bearer ", "")
			}
		};
		let state: AppState = AppState::from_ref(state);
		Self::parse(&token, &state).await
	}
}

pub async fn login(
	State(state): State<AppState>,
	Query(params): Query<HashMap<String, String>>,
	cookies: CookieJar,
) -> Result<(CookieJar, Redirect), StatusCode> {
	let code = params.get("code").ok_or(StatusCode::UNAUTHORIZED)?;

	let mut params: HashMap<&str, &str> = std::collections::HashMap::new();
	params.insert("grant_type", "authorization_code");
	params.insert("redirect_uri", "https://divamodarchive.com/login");
	params.insert("code", &code);

	#[derive(Serialize, Deserialize)]
	struct DiscordTokenResponse {
		access_token: String,
		token_type: String,
		expires_in: i64,
		refresh_token: String,
		scope: String,
	}

	#[derive(Serialize, Deserialize)]
	struct DiscordUser {
		id: String,
		global_name: String,
		discriminator: String,
		avatar: Option<String>,
	}

	let response = reqwest::Client::new()
		.post("https://discord.com/api/v10/oauth2/token")
		.basic_auth(state.config.discord_id, Some(state.config.discord_secret))
		.form(&params)
		.send()
		.await
		.map_err(|_| StatusCode::BAD_REQUEST)?;
	if !response.status().is_success() {
		return Err(StatusCode::BAD_REQUEST);
	};

	let response: DiscordTokenResponse = response
		.json()
		.await
		.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

	let response = reqwest::Client::new()
		.get("https://discord.com/api/users/@me")
		.header(
			"authorization",
			format!("{} {}", response.token_type, response.access_token),
		)
		.send()
		.await
		.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

	if !response.status().is_success() {
		return Err(StatusCode::INTERNAL_SERVER_ERROR);
	}

	let response: DiscordUser = response
		.json()
		.await
		.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
	let id: i64 = response
		.id
		.parse()
		.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
	let avatar = if let Some(avatar) = response.avatar {
		format!(
			"https://cdn.discordapp.com/avatars/{}/{}.png?size=512",
			id, avatar
		)
	} else {
		let discriminator: i32 = response.discriminator.parse().unwrap_or_default();
		format!(
			"https://cdn.discordapp.com/embed/avatars/{}.png?size=512",
			discriminator % 5
		)
	};
	sqlx::query!(
		"INSERT INTO users VALUES ($1, $2, $3) ON CONFLICT (id) DO UPDATE SET avatar = excluded.avatar, name = excluded.name",
		id,
		response.global_name,
		avatar
	)
	.execute(&state.db)
	.await
	.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

	let time = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.unwrap()
		.as_secs() as i64;
	let token = Token {
		exp: time + 60 * 24 * 60 * 60,
		user_id: id,
	};

	if let Ok(encoded) = encode(&Header::default(), &token, &state.config.encoding_key) {
		let mut cookie = Cookie::new(AUTHORIZATION.to_string(), encoded);
		cookie.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
		Ok((cookies.add(cookie), Redirect::to("/")))
	} else {
		Err(StatusCode::UNAUTHORIZED)
	}
}
