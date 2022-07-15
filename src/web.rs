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

pub enum Order {
	Latest,
	Popular,
}

#[get("/?<offset>&<name>&<order>")]
pub fn find_posts(
	connection: &ConnectionState,
	offset: Option<i64>,
	name: Option<String>,
	order: Option<String>,
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
		Order::Latest => "Latest",
		Order::Popular => "Popular",
	};
	let results = match sort_order {
		Order::Latest => get_latest_posts(connection, name.clone(), offset),
		Order::Popular => get_popular_posts(connection, name.clone(), offset),
	}
	.unwrap_or_default();
	let description = match sort_order {
		Order::Latest => "The latest posts on DIVA Mod Archive",
		Order::Popular => "The most popular posts on DIVA Mod Archive",
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
		],
	))
}

#[get("/posts/<id>")]
pub fn details(
	connection: &ConnectionState,
	id: i32,
	cookies: &CookieJar<'_>,
) -> Result<Template, Status> {
	// name=post.name, text=post.text, link=post.link, uploader_id=user.id, uploader_name=user.name,
	// likes=post.likes, dislikes=post.dislikes, is_logged_in=is_logged_in, has_liked=has_liked, has_disliked=has_disliked
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
			context![post: &post, is_logged_in: true, has_liked: has_liked, has_disliked: has_disliked, jwt: jwt.value(), who_is_logged_in: who_is_logged_in],
		))
	} else {
		Ok(Template::render(
			"post_detail",
			context![post: &post, is_logged_in: false, has_liked: false, has_disliked: false, jwt: None::<String>, who_is_logged_in: 0],
		))
	}
}

#[get("/login?<code>")]
pub async fn login(
	connection: &ConnectionState,
	code: String,
	cookies: &CookieJar<'_>,
) -> Redirect {
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
		context![user: user, is_logged_in: is_logged_in(connection, cookies), jwt: cookies.get_pending("jwt").unwrap().value()],
	))
}

#[get("/user/<id>?<offset>&<order>")]
pub fn user(
	connection: &ConnectionState,
	id: i64,
	offset: Option<i64>,
	order: Option<String>,
	cookies: &CookieJar<'_>,
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
		Order::Latest => "Latest",
		Order::Popular => "Popular",
	};
	let results = match sort_order {
		Order::Latest => get_user_posts_latest(connection, user.id, offset),
		Order::Popular => get_user_posts_popular(connection, user.id, offset),
	}
	.unwrap_or_default();
	let description = match sort_order {
		Order::Latest => format!("The latest posts by {}", user.name),
		Order::Popular => format!("The most popular posts by {}", user.name),
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
			total_likes: user_stats.0,
			total_dislikes: user_stats.1,
			total_downloads: user_stats.2
		],
	))
}

#[get("/posts/<id>/edit")]
pub fn edit(
	connection: &ConnectionState,
	id: i32,
	user: User,
	cookies: &CookieJar<'_>,
) -> Result<Template, Redirect> {
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
				context![user: user, is_logged_in: true, jwt: jwt.value(), previous_title: post.name, previous_description: post.text, previous_description_short: post.text_short, likes: post.likes, dislikes: post.dislikes],
			))
		} else {
			Err(Redirect::to(format!("/posts/{}", id)))
		}
	} else {
		Err(Redirect::to(format!("/posts/{}", id)))
	}
}
