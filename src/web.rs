use crate::models::*;
use crate::posts::*;
use crate::users::*;
use diesel::PgConnection;
use jsonwebtoken::*;
use rocket::http::*;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;

pub fn who_is_logged_in(
	connection: &mut PgConnection,
	cookies: &CookieJar<'_>,
) -> Result<User, Status> {
	let jwt = cookies.get_pending("jwt");
	if jwt.is_none() {
		return Err(Status::Unauthorized);
	}
	let jwt = jwt.unwrap();
	let jwt = jwt.value();
	let jwt_string = String::from(jwt);
	let token_data = decode::<Token>(&jwt, &DECODE_KEY, &Validation::default());
	if let Ok(token_data) = token_data {
		let result = get_user(connection, token_data.claims.user_id);
		if result.is_ok() {
			Ok(result.unwrap())
		} else {
			cookies.remove(Cookie::new("jwt", jwt_string));
			Err(Status::Unauthorized)
		}
	} else {
		cookies.remove(Cookie::new("jwt", jwt_string));
		Err(Status::Unauthorized)
	}
}

pub fn is_logged_in(connection: &mut PgConnection, cookies: &CookieJar<'_>) -> bool {
	let jwt = cookies.get_pending("jwt");
	if jwt.is_none() {
		return false;
	}
	let jwt = jwt.unwrap();
	let jwt = jwt.value();
	let jwt_string = String::from(jwt);
	let token_data = decode::<Token>(&jwt, &DECODE_KEY, &Validation::default());
	if let Ok(token_data) = token_data {
		let result = get_user(connection, token_data.claims.user_id).is_ok();
		if result {
			true
		} else {
			cookies.remove(Cookie::new("jwt", jwt_string));
			false
		}
	} else {
		cookies.remove(Cookie::new("jwt", jwt_string));
		false
	}
}

pub fn is_light_mode(cookies: &CookieJar<'_>) -> bool {
	let light_mode = cookies.get_pending("light_mode");
	if light_mode.is_none() {
		return false;
	}
	let light_mode = light_mode.unwrap();
	let light_mode = light_mode.value();
	light_mode == "true"
}

#[get("/theme")]
pub fn set_theme(cookies: &CookieJar<'_>) -> Redirect {
	let new = if is_light_mode(cookies) {
		"false"
	} else {
		"true"
	};
	cookies.add(Cookie::new("light_mode", new));
	Redirect::to("/")
}

pub enum Order {
	Latest,
	Popular,
}

#[get("/?<offset>&<name>&<order>&<game_tag>")]
pub fn find_posts(
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
			"latest" => Order::Latest,
			"popular" => Order::Popular,
			_ => Order::Latest,
		},
		None => Order::Latest,
	};
	let connection = &mut connection.lock().unwrap();
	let offset = offset.unwrap_or(0);
	let name = name.unwrap_or_default();
	let title = match sort_order {
		Order::Latest => "Latest DIVA Mods",
		Order::Popular => "Popular DIVA Mods",
	};
	let results = match sort_order {
		Order::Latest => get_latest_posts(connection, name.clone(), offset, game_tag.unwrap_or(0)),
		Order::Popular => {
			get_popular_posts(connection, name.clone(), offset, game_tag.unwrap_or(0))
		}
	};
	let description = match sort_order {
		Order::Latest => "The latest project DIVA mods",
		Order::Popular => "The most popular project DIVA mods",
	};
	Ok(Template::render(
		"post_list",
		context![
			posts: &results,
			is_logged_in: is_logged_in(connection, cookies),
			title: title,
			description: description,
			offset: offset,
			previous_search: name,
			previous_sort: order.unwrap_or_default(),
			previous_game_tag: game_tag.unwrap_or(0),
			light_mode: is_light_mode(cookies),
			game_tags: TAG_TOML.game_tags.clone(),
			type_tags: TAG_TOML.type_tags.clone(),
			is_admin: ADMINS.contains(&user.unwrap_or_default().id)
		],
	))
}

#[get("/posts/<id>")]
pub fn details(
	connection: &ConnectionState,
	id: i32,
	cookies: &CookieJar<'_>,
) -> Result<Template, Status> {
	let connection = &mut connection.lock().unwrap();
	let post = get_post(connection, id)?;
	let who_is_logged_in = who_is_logged_in(connection, cookies);
	if who_is_logged_in.is_ok() {
		let who_is_logged_in = who_is_logged_in.unwrap().id;
		let has_liked = has_liked_post(connection, who_is_logged_in, id);
		let has_disliked = has_disliked_post(connection, who_is_logged_in, id);
		let jwt = cookies.get_pending("jwt").unwrap();
		Ok(Template::render(
			"post_detail",
			context![post: &post, is_logged_in: true, has_liked: has_liked, has_disliked: has_disliked, jwt: jwt.value(), who_is_logged_in: who_is_logged_in, light_mode: is_light_mode(cookies), game_tags: TAG_TOML.game_tags.clone(), type_tags: TAG_TOML.type_tags.clone(), is_admin: ADMINS.contains(&who_is_logged_in)],
		))
	} else {
		Ok(Template::render(
			"post_detail",
			context![post: &post, is_logged_in: false, has_liked: false, has_disliked: false, jwt: None::<String>, who_is_logged_in: 0, light_mode: is_light_mode(cookies), game_tags: TAG_TOML.game_tags.clone(), type_tags: TAG_TOML.type_tags.clone()],
		))
	}
}

#[get("/login?<code>")]
pub async fn login(
	connection: &ConnectionState,
	code: Option<String>,
	cookies: &CookieJar<'_>,
) -> Redirect {
	if code.is_none() {
		return Redirect::to("/");
	}
	let code = code.unwrap();
	let jwt = crate::api::v1::users::login(
		connection,
		code,
		Some(format!("{}/login", BASE_URL.to_string())),
	)
	.await;
	if jwt.is_err() {
		Redirect::to("/")
	} else {
		let jwt = jwt.unwrap();
		let mut cookie = Cookie::new("jwt", jwt);
		cookie.set_same_site(SameSite::Lax);
		cookies.add(cookie);
		Redirect::to(uri!("/"))
	}
}

#[get("/upload")]
pub fn upload(
	connection: &ConnectionState,
	user: User,
	cookies: &CookieJar<'_>,
) -> Result<Template, Status> {
	let connection = &mut connection.lock().unwrap();
	Ok(Template::render(
		"upload",
		context![user: &user, is_logged_in: is_logged_in(connection, cookies), jwt: cookies.get_pending("jwt").unwrap().value(), light_mode: is_light_mode(cookies),base_url: BASE_URL.to_string(), game_tags: TAG_TOML.game_tags.clone(), type_tags: TAG_TOML.type_tags.clone(), is_admin: ADMINS.contains(&user.id)],
	))
}

#[get("/user/<id>?<offset>&<order>&<game_tag>")]
pub fn user(
	connection: &ConnectionState,
	id: i64,
	offset: Option<i64>,
	order: Option<String>,
	game_tag: Option<i32>,
	cookies: &CookieJar<'_>,
	current_user: Option<User>,
) -> Result<Template, Status> {
	let connection = &mut connection.lock().unwrap();
	let user = get_user(connection, id)?;
	let sort_order = match order.clone() {
		Some(order) => match order.as_str() {
			"latest" => Order::Latest,
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
		Order::Latest => get_user_posts_latest(connection, user.id, offset, game_tag.unwrap_or(0)),
		Order::Popular => {
			get_user_posts_popular(connection, user.id, offset, game_tag.unwrap_or(0))
		}
	}
	.unwrap_or_default();
	let description = match sort_order {
		Order::Latest => format!("The latest DIVA mods by {}", user.name),
		Order::Popular => format!("The most popular DIVA mods by {}", user.name),
	};
	let user_stats = get_user_stats(connection, user.id);

	let is_logged_in = is_logged_in(connection, cookies);
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
			light_mode: is_light_mode(cookies),
			game_tags: TAG_TOML.game_tags.clone(),
			type_tags: TAG_TOML.type_tags.clone(),
			is_admin: ADMINS.contains(&current_user.unwrap_or_default().id)
		],
	))
}

#[get("/posts/<id>/edit")]
pub fn edit(
	connection: &ConnectionState,
	id: i32,
	user: Option<User>,
	cookies: &CookieJar<'_>,
) -> Result<Template, Redirect> {
	if user.is_none() {
		return Err(Redirect::to(format!("/posts/{}", id)));
	}
	let user = user.unwrap();
	let connection = &mut connection.lock().unwrap();
	let post = get_post(connection, id);
	let who_is_logged_in = who_is_logged_in(connection, cookies);
	if post.is_ok() && who_is_logged_in.is_ok() {
		let post = post.unwrap();
		let who_is_logged_in = who_is_logged_in.unwrap().id;
		if post.user.id == who_is_logged_in {
			let jwt = cookies.get_pending("jwt").unwrap();
			Ok(Template::render(
				"upload",
				context![user: &user, is_logged_in: true, jwt: jwt.value(), previous_title: post.name, previous_description: post.text, previous_description_short: post.text_short, likes: post.likes, dislikes: post.dislikes, light_mode: is_light_mode(cookies), update_id: id, base_url: BASE_URL.to_string(), previous_game_tag: post.game_tag, previous_type_tag: post.type_tag, game_tags: TAG_TOML.game_tags.clone(),type_tags: TAG_TOML.type_tags.clone(), is_admin: ADMINS.contains(&user.id)],
			))
		} else {
			Err(Redirect::to(format!("/posts/{}", id)))
		}
	} else {
		Err(Redirect::to(format!("/posts/{}", id)))
	}
}

#[get("/posts/<id>/dependency?<offset>&<name>&<order>")]
pub fn dependency(
	connection: &ConnectionState,
	id: i32,
	offset: Option<i64>,
	name: Option<String>,
	order: Option<String>,
	user: User,
	cookies: &CookieJar<'_>,
) -> Result<Template, Redirect> {
	let connection = &mut connection.lock().unwrap();
	let post = get_post(connection, id);
	if post.is_err() {
		return Err(Redirect::to(format!("/posts/{}", id)));
	}
	let post = post.unwrap();
	if post.user.id != user.id {
		return Err(Redirect::to(format!("/posts/{}", id)));
	}

	let offset = offset.unwrap_or(0);
	let name = name.unwrap_or_default();

	let sort_order = match order.clone() {
		Some(order) => match order.as_str() {
			"latest" => Order::Latest,
			"popular" => Order::Popular,
			_ => Order::Latest,
		},
		None => Order::Latest,
	};
	let posts = match sort_order {
		Order::Latest => {
			get_latest_posts_disallowed(connection, name.clone(), offset, post.game_tag, vec![id])
		}
		Order::Popular => {
			get_popular_posts_disallowed(connection, name.clone(), offset, post.game_tag, vec![id])
		}
	}
	.unwrap_or_default();
	Ok(Template::render(
		"dependencies",
		context![
			id: id,
			posts: &posts,
			is_logged_in: true,
			light_mode: is_light_mode(cookies),
			previous_search: name,
			previous_sort: order.unwrap_or_default(),
			offset: offset,
			game_tags: TAG_TOML.game_tags.clone(),
			type_tags: TAG_TOML.type_tags.clone(),
			is_admin: ADMINS.contains(&user.id)
		],
	))
}

#[get("/posts/<id>/dependency/<dependency_id>")]
pub fn dependency_add(
	connection: &ConnectionState,
	id: i32,
	dependency_id: i32,
	user: User,
) -> Redirect {
	let connection = &mut connection.lock().unwrap();
	if owns_post(connection, id, user.id) {
		add_dependency(connection, id, dependency_id);
	}
	Redirect::to(format!("/posts/{}", id))
}

#[get("/posts/<id>/dependency/<dependency_id>/remove")]
pub fn dependency_remove(
	connection: &ConnectionState,
	id: i32,
	dependency_id: i32,
	user: User,
) -> Redirect {
	let connection = &mut connection.lock().unwrap();
	if owns_post(connection, id, user.id) {
		remove_dependency(connection, id, dependency_id);
	}
	Redirect::to(format!("/posts/{}", id))
}

#[get("/about")]
pub fn about(
	connection: &ConnectionState,
	cookies: &CookieJar<'_>,
	user: Option<User>,
) -> Template {
	Template::render(
		"about",
		context![
			is_logged_in: is_logged_in(&mut connection.lock().unwrap(), cookies),
			light_mode: is_light_mode(cookies),
			user: user.unwrap_or_default(),
		],
	)
}

#[get("/liked?<offset>")]
pub fn liked(
	connection: &ConnectionState,
	user: User,
	offset: Option<i64>,
	cookies: &CookieJar<'_>,
) -> Template {
	let connection = &mut connection.lock().unwrap();
	let posts = get_user_liked_posts(connection, user.id, offset.unwrap_or(0));
	Template::render(
		"liked",
		context![
			posts: &posts,
			is_logged_in: is_logged_in(connection, cookies),
			title: "Liked Mods",
			description: "Liked Mods",
			offset: offset,
			light_mode: is_light_mode(cookies),
			game_tags: TAG_TOML.game_tags.clone(),
			type_tags: TAG_TOML.type_tags.clone(),
			is_admin: ADMINS.contains(&user.id)
		],
	)
}

#[get("/logout")]
pub fn logout(cookies: &CookieJar<'_>) -> Redirect {
	let jwt = cookies.get_pending("jwt");
	if jwt.is_none() {
		return Redirect::to("/");
	}
	let jwt = jwt.unwrap();
	let jwt = jwt.value();
	let jwt_string = String::from(jwt);
	cookies.remove(Cookie::new("jwt", jwt_string));
	Redirect::to("/")
}

#[get("/admin")]
pub fn admin(
	connection: &ConnectionState,
	user: User,
	cookies: &CookieJar<'_>,
) -> Result<Template, Redirect> {
	let connection = &mut connection.lock().unwrap();
	if !ADMINS.contains(&user.id) {
		return Err(Redirect::to("/"));
	}
	Ok(Template::render(
		"admin",
		context![
			is_logged_in: true,
			light_mode: is_light_mode(cookies),
			game_tags: TAG_TOML.game_tags.clone(),
			type_tags: TAG_TOML.type_tags.clone(),
			is_admin: true,
			reports: get_reports(connection),
			posts: get_latest_posts_unfiltered(connection)
		],
	))
}

#[get("/posts/<id>/remove")]
pub fn remove_post_admin(connection: &ConnectionState, user: User, id: i32) -> Redirect {
	let connection = &mut connection.lock().unwrap();
	if !ADMINS.contains(&user.id) {
		return Redirect::to("/");
	}
	delete_post(connection, id);
	Redirect::to("/admin")
}

#[get("/report/<id>/remove")]
pub fn remove_report(connection: &ConnectionState, user: User, id: i32) -> Redirect {
	let connection = &mut connection.lock().unwrap();
	if !ADMINS.contains(&user.id) {
		return Redirect::to("/");
	}
	delete_report(connection, id);
	Redirect::to("/admin")
}

// Button on post detail if logged in leads here
// Use to send a report against a post
#[allow(unused_variables)]
#[get("/posts/<id>/report")]
pub fn report(
	connection: &ConnectionState,
	user: User,
	id: i32,
	cookies: &CookieJar<'_>,
) -> Result<Template, Redirect> {
	let connection = &mut connection.lock().unwrap();
	let post = get_post(connection, id);
	if post.is_err() {
		return Err(Redirect::to("/"));
	}
	let post = post.unwrap();
	Ok(Template::render(
		"report",
		context![
			is_logged_in: is_logged_in(connection, cookies),
			light_mode: is_light_mode(cookies),
			user: &user,
			post: &post,
			game_tags: TAG_TOML.game_tags.clone(),
			type_tags: TAG_TOML.type_tags.clone(),
			is_admin: ADMINS.contains(&user.id)
		],
	))
}

// Sends a report
// Put in database then redirect to the mod
#[allow(unused_variables)]
#[post("/posts/<id>/report_send", data = "<reason>")]
pub fn report_send(
	connection: &ConnectionState,
	user: User,
	id: i32,
	reason: String,
	cookies: &CookieJar<'_>,
) -> Redirect {
	let reason = reason.replace("reason=", "");
	let connection = &mut connection.lock().unwrap();
	let _ = add_report(connection, id, user.id, reason);
	Redirect::to("/")
}
