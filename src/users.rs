use super::models::*;
use super::schema::*;
use diesel::dsl::*;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use rocket::http::Status;

pub fn create_user<'a>(
	conn: &mut PgConnection,
	id: i64,
	name: &'a str,
	avatar: &'a str,
) -> Result<User, Status> {
	// Check if entry with same user id already exists, if so update name and avatar
	let user = users::table
		.filter(users::user_id.eq(id))
		.first::<User>(conn);
	if user.is_ok() {
		let result = diesel::update(users::table.filter(users::user_id.eq(id)))
			.set((users::user_name.eq(name), users::user_avatar.eq(avatar)))
			.get_result(conn);
		match result {
			Ok(user) => Ok(user),
			Err(_) => Err(Status::InternalServerError),
		}
	} else {
		let new_user = NewUser {
			user_id: id,
			user_name: name,
			user_avatar: avatar,
		};
		let result = diesel::insert_into(users::table)
			.values(&new_user)
			.get_result(conn);
		match result {
			Ok(user) => Ok(user),
			Err(_) => Err(Status::InternalServerError),
		}
	}
}

// Ensure the user is verified before calling this
pub fn delete_user(conn: &mut PgConnection, id: i64) -> Status {
	let result = diesel::delete(users::table.filter(users::user_id.eq(id))).execute(conn);

	if result.is_ok() {
		Status::Ok
	} else {
		Status::InternalServerError
	}
}

pub fn get_user(conn: &mut PgConnection, id: i64) -> Result<User, Status> {
	let result = users::table.filter(users::user_id.eq(id)).get_result(conn);

	match result {
		Ok(user) => Ok(user),
		Err(_) => Err(Status::InternalServerError),
	}
}

pub fn get_user_posts_latest(
	conn: &mut PgConnection,
	id: i64,
	offset: i64,
	game_tag: i32,
	limit: i64,
) -> Vec<ShortUserPosts> {
	users::table
		.filter(users::user_id.eq(id))
		.inner_join(posts::table)
		.filter(posts::post_game_tag.eq(game_tag))
		.left_join(users_liked_posts::table.on(users_liked_posts::post_id.eq(posts::post_id)))
		.left_join(users_disliked_posts::table.on(users_disliked_posts::post_id.eq(posts::post_id)))
		.left_join(download_stats::table.on(download_stats::post_id.eq(posts::post_id)))
		.group_by((posts::post_id, users::user_id))
		.order_by(posts::post_date.desc())
		.select((
			(
				posts::post_id,
				posts::post_name,
				posts::post_text_short,
				posts::post_image,
				posts::post_game_tag,
				posts::post_type_tag,
				count_distinct(users_liked_posts::user_id.nullable()),
				count_distinct(users_disliked_posts::user_id.nullable()),
				count_distinct(download_stats::timestamp.nullable()),
			),
			(users::user_id, users::user_name, users::user_avatar),
		))
		.limit(limit)
		.offset(offset)
		.load::<ShortUserPosts>(conn)
		.unwrap_or_else(|_| vec![])
}

pub fn get_user_posts_popular(
	conn: &mut PgConnection,
	id: i64,
	offset: i64,
	game_tag: i32,
	limit: i64,
) -> Vec<ShortUserPosts> {
	users::table
		.filter(users::user_id.eq(id))
		.inner_join(posts::table)
		.filter(posts::post_game_tag.eq(game_tag))
		.left_join(users_liked_posts::table.on(users_liked_posts::post_id.eq(posts::post_id)))
		.left_join(users_disliked_posts::table.on(users_disliked_posts::post_id.eq(posts::post_id)))
		.left_join(download_stats::table.on(download_stats::post_id.eq(posts::post_id)))
		.group_by((posts::post_id, users::user_id))
		.order_by(
			(count_distinct(users_liked_posts::user_id.nullable())
				- count_distinct(users_disliked_posts::user_id.nullable()))
			.desc(),
		)
		.then_order_by(count_distinct(download_stats::timestamp.nullable()).desc())
		.select((
			(
				posts::post_id,
				posts::post_name,
				posts::post_text_short,
				posts::post_image,
				posts::post_game_tag,
				posts::post_type_tag,
				count_distinct(users_liked_posts::user_id.nullable()),
				count_distinct(users_disliked_posts::user_id.nullable()),
				count_distinct(download_stats::timestamp.nullable()),
			),
			(users::user_id, users::user_name, users::user_avatar),
		))
		.limit(limit)
		.offset(offset)
		.load::<ShortUserPosts>(conn)
		.unwrap_or_else(|_| vec![])
}

pub fn get_user_stats(conn: &mut PgConnection, id: i64) -> UserStats {
	users::table
		.filter(users::user_id.eq(id))
		.inner_join(posts::table)
		.left_join(users_liked_posts::table.on(users_liked_posts::post_id.eq(posts::post_id)))
		.left_join(users_disliked_posts::table.on(users_disliked_posts::post_id.eq(posts::post_id)))
		.left_join(download_stats::table.on(download_stats::post_id.eq(posts::post_id)))
		.group_by((posts::post_id, users::user_id))
		.select((
			count_distinct(users_liked_posts::user_id.nullable()),
			count_distinct(users_disliked_posts::user_id.nullable()),
			count_distinct(download_stats::timestamp.nullable()),
		))
		.get_result::<UserStats>(conn)
		.unwrap_or_default()
}

pub fn get_user_liked_posts(
	conn: &mut PgConnection,
	id: i64,
	offset: i64,
	limit: i64,
) -> Vec<ShortPostNoLikes> {
	users_liked_posts::table
		.filter(users_liked_posts::user_id.eq(id))
		.inner_join(posts::table)
		.left_join(download_stats::table.on(download_stats::post_id.eq(posts::post_id)))
		.group_by(posts::post_id)
		.order_by(posts::post_date.desc())
		.select((
			posts::post_id,
			posts::post_name,
			posts::post_text_short,
			posts::post_image,
			posts::post_game_tag,
			posts::post_type_tag,
			count_distinct(download_stats::timestamp.nullable()),
		))
		.limit(limit)
		.offset(offset)
		.load::<ShortPostNoLikes>(conn)
		.unwrap_or_else(|_| vec![])
}
