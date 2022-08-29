use crate::models::*;
use crate::posts::*;
use crate::users::*;
use diesel::PgConnection;
use jsonwebtoken::*;
use rocket::http::*;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;

pub async fn who_is_logged_in(
	connection: &mut PgConnection,
	cookies: &CookieJar<'_>,
) -> Result<User, Status> {
	let jwt = cookies.get_pending("jwt");
	let jwt = match jwt {
		Some(jwt) => jwt,
		None => return Err(Status::Unauthorized),
	};
	let jwt = jwt.value();
	let token_data = decode::<Token>(jwt, &DECODE_KEY, &Validation::default());
	let token_data = match token_data {
		Ok(token_data) => token_data,
		Err(_) => return Err(Status::Unauthorized),
	};
	let result = get_user(connection, token_data.claims.user_id).await;
	match result {
		Ok(user) => Ok(user),
		Err(_) => Err(Status::Unauthorized),
	}
}

pub async fn is_logged_in(connection: &mut PgConnection, cookies: &CookieJar<'_>) -> bool {
	let jwt = cookies.get_pending("jwt");
	let jwt = match jwt {
		Some(jwt) => jwt,
		None => return false,
	};
	let jwt = jwt.value();
	let token_data = decode::<Token>(jwt, &DECODE_KEY, &Validation::default());
	let token_data = match token_data {
		Ok(token_data) => token_data,
		Err(_) => return false,
	};
	get_user(connection, token_data.claims.user_id)
		.await
		.is_ok()
}

pub async fn get_theme(cookies: &CookieJar<'_>) -> Theme {
	let theme = cookies.get_pending("theme");
	let theme_id = theme.map_or(0, |theme| theme.value().parse::<i32>().unwrap_or(0));
	let theme_result = THEMES_TOML.themes.iter().find(|theme| theme.id == theme_id);
	theme_result.map_or_else(Theme::default, std::clone::Clone::clone)
}

#[get("/theme")]
pub async fn set_theme(cookies: &CookieJar<'_>) -> Redirect {
	let current_id = get_theme(cookies).await.id;
	let ids = THEMES_TOML
		.themes
		.iter()
		.map(|x| x.id)
		.collect::<Vec<i32>>();
	let max = ids.iter().max().unwrap_or(&0);
	let new = if &current_id == max {
		0
	} else {
		current_id + 1
	};
	cookies.add(Cookie::new("theme", new.to_string()));
	Redirect::to("/")
}

pub enum Order {
	Latest,
	Popular,
}

#[get("/?<offset>&<name>&<order>&<game_tag>")]
pub async fn find_posts(
	connection: &ConnectionState,
	offset: Option<i64>,
	name: Option<String>,
	order: Option<String>,
	game_tag: Option<i32>,
	user: Option<User>,
	cookies: &CookieJar<'_>,
) -> Result<Template, Status> {
	let sort_order = match order.clone() {
		Some(order) => match order.as_str() {
			"popular" => Order::Popular,
			_ => Order::Latest,
		},
		None => Order::Latest,
	};
	let connection = &mut get_connection(connection).await;
	let offset = offset.unwrap_or(0);
	let name = name.unwrap_or_default();
	let title = match sort_order {
		Order::Latest => format!("Latest {} Mods", *GAME_NAME),
		Order::Popular => format!("Popular {} Mods", *GAME_NAME),
	};
	let results = match sort_order {
		Order::Latest => {
			get_latest_posts(
				connection,
				name.clone(),
				offset,
				game_tag.unwrap_or(0),
				*WEBUI_LIMIT,
			)
			.await
		}
		Order::Popular => {
			get_popular_posts(
				connection,
				name.clone(),
				offset,
				game_tag.unwrap_or(0),
				*WEBUI_LIMIT,
			)
			.await
		}
	};
	let description = match sort_order {
		Order::Latest => format!("The latest {} Mods", *GAME_NAME),
		Order::Popular => format!("The most popular {} Mods", *GAME_NAME),
	};
	Ok(Template::render(
		"post_list",
		context![
			posts: &results,
			is_logged_in: is_logged_in(connection, cookies).await,
			title: title,
			description: description,
			offset: offset,
			previous_search: name,
			previous_sort: order.unwrap_or_default(),
			previous_game_tag: game_tag.unwrap_or(0),
			theme: get_theme(cookies).await,
			game_tags: TAG_TOML.game_tags.clone(),
			type_tags: TAG_TOML.type_tags.clone(),
			is_admin: ADMINS.contains(&user.unwrap_or_default().id),
			base_url: BASE_URL.to_string(),
			gtag: GTAG.to_string(),
			game_name: GAME_NAME.to_string(),
		],
	))
}

#[get("/posts/<id>")]
pub async fn details(
	connection: &ConnectionState,
	id: i32,
	cookies: &CookieJar<'_>,
) -> Result<Template, Status> {
	let connection = &mut get_connection(connection).await;
	let post = get_post(connection, id).await?;
	let who_is_logged_in = who_is_logged_in(connection, cookies).await;
	if let Ok(who_is_logged_in) = who_is_logged_in {
		let who_is_logged_in = who_is_logged_in.id;
		let has_liked = has_liked_post(connection, who_is_logged_in, id).await;
		let has_disliked = has_disliked_post(connection, who_is_logged_in, id).await;
		let jwt = cookies.get_pending("jwt").unwrap();
		Ok(Template::render(
			"post_detail",
			context![post: &post, is_logged_in: true, has_liked: has_liked, has_disliked: has_disliked, jwt: jwt.value(), who_is_logged_in: who_is_logged_in, theme: get_theme(cookies).await, game_tags: TAG_TOML.game_tags.clone(), type_tags: TAG_TOML.type_tags.clone(), is_admin: ADMINS.contains(&who_is_logged_in), base_url: BASE_URL.to_string(), gtag: GTAG.to_string(), game_name: GAME_NAME.to_string(),],
		))
	} else {
		Ok(Template::render(
			"post_detail",
			context![post: &post, is_logged_in: false, has_liked: false, has_disliked: false, jwt: None::<String>, who_is_logged_in: 0, theme: get_theme(cookies).await, game_tags: TAG_TOML.game_tags.clone(), type_tags: TAG_TOML.type_tags.clone(), is_admin: false, base_url: BASE_URL.to_string(), gtag: GTAG.to_string(), game_name: GAME_NAME.to_string(),],
		))
	}
}

#[get("/login?<code>")]
pub async fn login(
	connection: &ConnectionState,
	code: Option<String>,
	cookies: &CookieJar<'_>,
) -> Redirect {
	let code = match code {
		Some(code) => code,
		None => return Redirect::to("/"),
	};
	let jwt =
		crate::api::v1::users::login(connection, code, Some(format!("{}/login", *BASE_URL))).await;
	let jwt = match jwt {
		Ok(jwt) => jwt,
		Err(_) => return Redirect::to("/"),
	};
	let mut cookie = Cookie::new("jwt", jwt);
	cookie.set_same_site(SameSite::Lax);
	cookies.add(cookie);
	Redirect::to("/")
}

#[get("/upload")]
pub async fn upload(
	connection: &ConnectionState,
	user: User,
	cookies: &CookieJar<'_>,
) -> Result<Template, Status> {
	let connection = &mut get_connection(connection).await;
	Ok(Template::render(
		"upload",
		context![user: &user, is_logged_in: is_logged_in(connection, cookies).await, jwt: cookies.get_pending("jwt").unwrap().value(), theme: get_theme(cookies).await,base_url: BASE_URL.to_string(), game_tags: TAG_TOML.game_tags.clone(), type_tags: TAG_TOML.type_tags.clone(), is_admin: ADMINS.contains(&user.id), gtag: GTAG.to_string(), game_name: GAME_NAME.to_string(),],
	))
}

#[get("/user/<id>?<offset>&<order>&<game_tag>")]
pub async fn user(
	connection: &ConnectionState,
	id: i64,
	offset: Option<i64>,
	order: Option<String>,
	game_tag: Option<i32>,
	cookies: &CookieJar<'_>,
	current_user: Option<User>,
) -> Result<Template, Status> {
	let connection = &mut get_connection(connection).await;
	let user = get_user(connection, id).await?;
	let sort_order = match order.clone() {
		Some(order) => match order.as_str() {
			"popular" => Order::Popular,
			_ => Order::Latest,
		},
		None => Order::Latest,
	};
	let offset = offset.unwrap_or(0);
	let title = match sort_order {
		Order::Latest => format!("Mods by {}", user.name),
		Order::Popular => format!("Popular mods by {}", user.name),
	};
	let results = match sort_order {
		Order::Latest => {
			get_user_posts_latest(
				connection,
				user.id,
				offset,
				game_tag.unwrap_or(0),
				*WEBUI_LIMIT,
			)
			.await
		}
		Order::Popular => {
			get_user_posts_popular(
				connection,
				user.id,
				offset,
				game_tag.unwrap_or(0),
				*WEBUI_LIMIT,
			)
			.await
		}
	};
	let description = match sort_order {
		Order::Latest => format!("The latest {} mods by {}", *GAME_NAME, user.name),
		Order::Popular => format!("The most popular {} mods by {}", *GAME_NAME, user.name),
	};

	let user_stats = get_user_stats(connection, user.id).await;
	let is_logged_in = is_logged_in(connection, cookies).await;

	Ok(Template::render(
		"user_detail",
		context![
			user_posts: &results,
			is_logged_in: is_logged_in,
			title: title,
			description: description,
			offset: offset,
			previous_sort: order.unwrap_or_default(),
			previous_game_tag: game_tag.unwrap_or(0),
			total_likes: user_stats.likes,
			total_dislikes: user_stats.dislikes,
			total_downloads: user_stats.downloads,
			theme: get_theme(cookies).await,
			game_tags: TAG_TOML.game_tags.clone(),
			type_tags: TAG_TOML.type_tags.clone(),
			is_admin: ADMINS.contains(&current_user.unwrap_or_default().id),
			base_url: BASE_URL.to_string(),
			gtag: GTAG.to_string(),
			game_name: GAME_NAME.to_string(),
		],
	))
}

#[get("/posts/<id>/edit")]
pub async fn edit(
	connection: &ConnectionState,
	id: i32,
	user: User,
	cookies: &CookieJar<'_>,
) -> Result<Template, Redirect> {
	let connection = &mut get_connection(connection).await;
	let post = match get_post(connection, id).await {
		Ok(post) => post,
		Err(_) => return Err(Redirect::to(format!("/posts/{}", id))),
	};
	let who_is_logged_in = match who_is_logged_in(connection, cookies).await {
		Ok(who_is_logged_in) => who_is_logged_in,
		Err(_) => return Err(Redirect::to(format!("/posts/{}", id))),
	};
	let who_is_logged_in = who_is_logged_in.id;
	if post.user.id == who_is_logged_in {
		let jwt = cookies.get_pending("jwt").unwrap();
		Ok(Template::render(
			"upload",
			context![user: &user, is_logged_in: true, jwt: jwt.value(), previous_title: post.name, previous_description: post.text, previous_description_short: post.text_short, likes: post.likes, dislikes: post.dislikes, theme: get_theme(cookies).await, update_id: id, base_url: BASE_URL.to_string(), previous_game_tag: post.game_tag, previous_type_tag: post.type_tag, game_tags: TAG_TOML.game_tags.clone(),type_tags: TAG_TOML.type_tags.clone(), is_admin: ADMINS.contains(&user.id), gtag: GTAG.to_string(), game_name: GAME_NAME.to_string(),],
		))
	} else {
		Err(Redirect::to(format!("/posts/{}", id)))
	}
}

#[get("/posts/<id>/dependency?<offset>&<name>&<order>")]
pub async fn dependency(
	connection: &ConnectionState,
	id: i32,
	offset: Option<i64>,
	name: Option<String>,
	order: Option<String>,
	user: User,
	cookies: &CookieJar<'_>,
) -> Result<Template, Redirect> {
	let connection = &mut get_connection(connection).await;
	let post = match get_post(connection, id).await {
		Ok(post) => post,
		Err(_) => return Err(Redirect::to(format!("/posts/{}", id))),
	};
	if post.user.id != user.id {
		return Err(Redirect::to(format!("/posts/{}", id)));
	}

	let offset = offset.unwrap_or(0);
	let name = name.unwrap_or_default();

	let sort_order = match order.clone() {
		Some(order) => match order.as_str() {
			"popular" => Order::Popular,
			_ => Order::Latest,
		},
		None => Order::Latest,
	};
	let posts = match sort_order {
		Order::Latest => {
			get_latest_posts_disallowed(
				connection,
				name.clone(),
				offset,
				post.game_tag,
				vec![id],
				*WEBUI_LIMIT,
			)
			.await
		}
		Order::Popular => {
			get_popular_posts_disallowed(
				connection,
				name.clone(),
				offset,
				post.game_tag,
				vec![id],
				*WEBUI_LIMIT,
			)
			.await
		}
	}
	.unwrap_or_default();
	Ok(Template::render(
		"dependencies",
		context![
			id: id,
			posts: &posts,
			is_logged_in: true,
			theme: get_theme(cookies).await,
			previous_search: name,
			previous_sort: order.unwrap_or_default(),
			offset: offset,
			game_tags: TAG_TOML.game_tags.clone(),
			type_tags: TAG_TOML.type_tags.clone(),
			is_admin: ADMINS.contains(&user.id),
			base_url: BASE_URL.to_string(),
			gtag: GTAG.to_string(),
			game_name: GAME_NAME.to_string(),
		],
	))
}

#[get("/posts/<id>/dependency/<dependency_id>")]
pub async fn dependency_add(
	connection: &ConnectionState,
	id: i32,
	dependency_id: i32,
	user: User,
) -> Redirect {
	let connection = &mut get_connection(connection).await;
	if owns_post(connection, id, user.id).await {
		add_dependency(connection, id, dependency_id).await;
	}
	Redirect::to(format!("/posts/{}", id))
}

#[get("/posts/<id>/dependency/<dependency_id>/remove")]
pub async fn dependency_remove(
	connection: &ConnectionState,
	id: i32,
	dependency_id: i32,
	user: User,
) -> Redirect {
	let connection = &mut get_connection(connection).await;
	if owns_post(connection, id, user.id).await {
		remove_dependency(connection, id, dependency_id).await;
	}
	Redirect::to(format!("/posts/{}", id))
}

#[get("/about")]
pub async fn about(
	connection: &ConnectionState,
	cookies: &CookieJar<'_>,
	user: Option<User>,
) -> Template {
	let connection = &mut get_connection(connection).await;
	Template::render(
		"about",
		context![
			is_logged_in: is_logged_in(connection, cookies).await,
			theme: get_theme(cookies).await,
			is_admin: ADMINS.contains(&user.unwrap_or_default().id),
			base_url: BASE_URL.to_string(),
			gtag: GTAG.to_string(),
			game_name: GAME_NAME.to_string(),
		],
	)
}

#[get("/liked?<offset>")]
pub async fn liked(
	connection: &ConnectionState,
	user: User,
	offset: Option<i64>,
	cookies: &CookieJar<'_>,
) -> Template {
	let connection = &mut get_connection(connection).await;
	let posts = get_user_liked_posts(connection, user.id, offset.unwrap_or(0), *WEBUI_LIMIT).await;
	Template::render(
		"liked",
		context![
			posts: &posts,
			is_logged_in: is_logged_in(connection, cookies).await,
			title: "Liked Mods",
			description: "Liked Mods",
			offset: offset,
			theme: get_theme(cookies).await,
			game_tags: TAG_TOML.game_tags.clone(),
			type_tags: TAG_TOML.type_tags.clone(),
			is_admin: ADMINS.contains(&user.id),
			base_url: BASE_URL.to_string(),
			gtag: GTAG.to_string(),
			game_name: GAME_NAME.to_string(),
		],
	)
}

#[get("/logout")]
pub async fn logout(cookies: &CookieJar<'_>) -> Redirect {
	let jwt = cookies.get_pending("jwt");
	if let Some(jwt) = jwt {
		let jwt = jwt.value();
		let jwt_string = String::from(jwt);
		cookies.remove(Cookie::new("jwt", jwt_string));
	}
	Redirect::to("/")
}

#[get("/admin")]
pub async fn admin(
	connection: &ConnectionState,
	user: User,
	cookies: &CookieJar<'_>,
) -> Result<Template, Redirect> {
	if !ADMINS.contains(&user.id) {
		return Err(Redirect::to("/"));
	}
	let connection = &mut get_connection(connection).await;
	Ok(Template::render(
		"admin",
		context![
			is_logged_in: true,
			theme: get_theme(cookies).await,
			game_tags: TAG_TOML.game_tags.clone(),
			type_tags: TAG_TOML.type_tags.clone(),
			is_admin: true,
			reports: get_reports(connection).await,
			posts: get_latest_posts_unfiltered(connection, *WEBUI_LIMIT).await,
			base_url: BASE_URL.to_string(),
			gtag: GTAG.to_string(),
			game_name: GAME_NAME.to_string(),
		],
	))
}

#[get("/posts/<id>/remove")]
pub async fn remove_post_admin(connection: &ConnectionState, user: User, id: i32) -> Redirect {
	if !ADMINS.contains(&user.id) {
		return Redirect::to("/");
	}
	let connection = &mut get_connection(connection).await;
	delete_post(connection, id).await;
	Redirect::to("/admin")
}

#[get("/report/<id>/remove")]
pub async fn remove_report(connection: &ConnectionState, user: User, id: i32) -> Redirect {
	if !ADMINS.contains(&user.id) {
		return Redirect::to("/");
	}
	let connection = &mut get_connection(connection).await;
	delete_report(connection, id).await;
	Redirect::to("/admin")
}

// Button on post detail if logged in leads here
// Use to send a report against a post
#[allow(unused_variables)]
#[get("/posts/<id>/report")]
pub async fn report(
	connection: &ConnectionState,
	user: User,
	id: i32,
	cookies: &CookieJar<'_>,
) -> Result<Template, Redirect> {
	let connection = &mut get_connection(connection).await;
	let post = match get_post(connection, id).await {
		Ok(post) => post,
		Err(_) => return Err(Redirect::to("/")),
	};
	Ok(Template::render(
		"report",
		context![
			is_logged_in: is_logged_in(connection, cookies).await,
			theme: get_theme(cookies).await,
			user: &user,
			post: &post,
			game_tags: TAG_TOML.game_tags.clone(),
			type_tags: TAG_TOML.type_tags.clone(),
			is_admin: ADMINS.contains(&user.id),
			base_url: BASE_URL.to_string(),
			gtag: GTAG.to_string(),
			game_name: GAME_NAME.to_string(),
		],
	))
}

// Sends a report
// Put in database then redirect to the mod
#[allow(unused_variables)]
#[post("/posts/<id>/report_send", data = "<reason>")]
pub async fn report_send(
	connection: &ConnectionState,
	user: User,
	id: i32,
	reason: &str,
	cookies: &CookieJar<'_>,
) -> Redirect {
	let reason = reason.replace("reason=", "");
	let connection = &mut get_connection(connection).await;
	let _ = add_report(connection, id, user.id, reason).await;
	Redirect::to("/")
}

#[get("/posts/<id>/comments/new?<text>")]
pub async fn create_comment(
	connection: &ConnectionState,
	user: User,
	id: i32,
	text: String,
) -> Redirect {
	let connection = &mut get_connection(connection).await;
	let _ = add_comment(connection, user.id, id, text, None).await;
	Redirect::to(format!("/posts/{}", id))
}

#[get("/posts/<id>/comments/<comment_id>/reply?<text>")]
pub async fn reply_comment(
	connection: &ConnectionState,
	user: User,
	id: i32,
	comment_id: i32,
	text: String,
) -> Redirect {
	let connection = &mut get_connection(connection).await;
	let _ = add_comment(connection, user.id, id, text, Some(comment_id)).await;
	Redirect::to(format!("/posts/{}", id))
}

#[get("/posts/<id>/comments/<comment_id>/remove")]
pub async fn remove_comment(
	connection: &ConnectionState,
	user: User,
	id: i32,
	comment_id: i32,
) -> Redirect {
	let connection = &mut get_connection(connection).await;
	let _ = delete_comment(connection, comment_id, user.id).await;
	Redirect::to(format!("/posts/{}", id))
}
