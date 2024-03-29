use crate::database::*;
use crate::models::*;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Default)]
struct DiscordTokenResponse {
	access_token: String,
	token_type: String,
	expires_in: i64,
	refresh_token: String,
	scope: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct DiscordUser {
	id: String,
	username: String,
	discriminator: String,
	avatar: Option<String>,
}

// Input:
// Code is a value gotten from a discord oauth2 url
// Output:
// Returns a jwt the user can use to authenticate with the server
// Behavior:
// Send http POST request to https://discord.com/api/v10/oauth2/token
// Data:
// client_id = env::var("DISCORD_ID")
// client_secret = env::var("DISCORD_SECRET")
// grant_type = "authorization_code"
// code = code
// redirect_uri = redirect_uri
// Header:
// Content-Type = application/x-www-form-urlencoded
// Returns token
// Use token to get the user id, username, and avatar link
// Send http GET request to https://discord.com/api/users/@me
// Header:
// authorization = token
// Returns id, username, avatar
// To get avatar URI https://cdn.discordapp.com/avatars/${id}/${avatar}.png
// If the avatar hash is none or empty, use the default avatar
// The default avatar can be gotten from https://cdn.discordapp.com/embed/avatars/${discriminator % 5}.png
// Adds the found user to the database if they are not already in it
// Updates user if they are in it
// Returns json web token for authentication
#[get("/login?<code>&<redirect_uri>")]
pub async fn login(
	connection: &ConnectionState,
	code: String,
	redirect_uri: Option<String>,
	config: &State<Config>,
) -> Result<String, Status> {
	let mut params = std::collections::HashMap::new();
	params.insert("client_id", config.discord_id.clone());
	params.insert("client_secret", config.discord_secret.clone());
	params.insert("grant_type", String::from("authorization_code"));
	params.insert("code", code);
	params.insert(
		"redirect_uri",
		redirect_uri.unwrap_or(format!("{}/api/v1/users/login", config.base_url)),
	);
	let response = reqwest::Client::new()
		.post("https://discord.com/api/v10/oauth2/token")
		.form(&params)
		.send()
		.await;
	let response = match response {
		Ok(response) => response,
		Err(_) => return Err(Status::BadRequest),
	};
	if !response.status().is_success() {
		return Err(Status::BadRequest);
	};
	let response: DiscordTokenResponse = response.json().await.unwrap_or_default();
	let response = reqwest::Client::new()
		.get("https://discord.com/api/users/@me")
		.header(
			"authorization",
			format!("{} {}", response.token_type, response.access_token),
		)
		.send()
		.await;

	let response = match response {
		Ok(response) => response,
		Err(_) => return Err(Status::BadRequest),
	};
	if !response.status().is_success() {
		return Err(Status::BadRequest);
	}

	let response: DiscordUser = response.json().await.unwrap_or_default();
	let id: i64 = response.id.parse().unwrap_or_default();
	let avatar = if let Some(avatar) = response.avatar {
		format!("https://cdn.discordapp.com/avatars/{}/{}.png", id, avatar)
	} else {
		let discriminator: i32 = response.discriminator.parse().unwrap_or_default();
		format!(
			"https://cdn.discordapp.com/embed/avatars/{}.png",
			discriminator % 5
		)
	};
	let connection = &mut get_connection(connection);
	let result = create_user(connection, id, &response.username, &avatar);
	match result {
		Some(_) => Ok(create_jwt(id, config)),
		None => Err(Status::BadRequest),
	}
}

#[get("/<id>")]
pub fn details(connection: &ConnectionState, id: i64) -> Option<Json<User>> {
	let connection = &mut get_connection(connection);
	let result = get_user(connection, id)?;
	Some(Json(result))
}

#[get("/<id>/latest?<offset>&<game_tag>&<limit>")]
pub fn latest(
	connection: &ConnectionState,
	id: i64,
	offset: Option<i64>,
	game_tag: Option<i32>,
	limit: Option<i64>,
	config: &State<Config>,
) -> Option<Json<Vec<ShortUserPosts>>> {
	let connection = &mut get_connection(connection);
	let result = get_user_posts_latest(
		connection,
		id,
		offset.unwrap_or(0),
		game_tag.unwrap_or(0),
		limit.unwrap_or(config.webui_limit),
	)?;
	Some(Json(result))
}

#[get("/<id>/popular?<offset>&<game_tag>&<limit>")]
pub fn popular(
	connection: &ConnectionState,
	id: i64,
	offset: Option<i64>,
	game_tag: Option<i32>,
	limit: Option<i64>,
	config: &State<Config>,
) -> Option<Json<Vec<ShortUserPosts>>> {
	let connection = &mut get_connection(connection);
	let result = get_user_posts_popular(
		connection,
		id,
		offset.unwrap_or(0),
		game_tag.unwrap_or(0),
		limit.unwrap_or(config.webui_limit),
	)?;
	Some(Json(result))
}

#[delete("/delete")]
pub fn delete(connection: &ConnectionState, user: User) {
	let connection = &mut get_connection(connection);
	delete_user(connection, user.id);
}
