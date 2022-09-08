use crate::models::*;
use crate::schema::*;
use bigdecimal::ToPrimitive;
use diesel::dsl::*;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use rocket::http::Status;
use rocket::*;
use std::net::IpAddr;

pub fn create_post(
	conn: &mut PgConnection,
	post: PostUnidentified,
	user: User,
	update_id: i32,
) -> Result<Post, Status> {
	if update_id != -1 {
		if !owns_post(conn, update_id, user.id) {
			return Err(Status::Unauthorized);
		}

		let original_post = get_post(conn, update_id);
		let original_post = match original_post {
			Some(post) => post,
			None => return Err(Status::BadRequest),
		};
		let image = match post.image {
			Some(image) => image,
			None => original_post.image,
		};
		let images_extra = match post.images_extra {
			Some(images) => images,
			None => original_post.images_extra,
		};

		let result = diesel::update(posts::table.filter(posts::post_id.eq(update_id)))
			.set((
				posts::post_name.eq(&post.name),
				posts::post_text.eq(&post.text),
				posts::post_text_short.eq(&post.text_short),
				posts::post_image.eq(image),
				posts::post_images_extra.eq(images_extra),
				posts::post_link.eq(&post.link),
				posts::post_date.eq(chrono::NaiveDateTime::from_timestamp(
					chrono::Utc::now().timestamp(),
					0,
				)),
				posts::post_game_tag.eq(post.game_tag),
				posts::post_type_tag.eq(post.type_tag),
				posts::post_downloads.eq(original_post.downloads),
			))
			.get_result::<Post>(conn);

		match result {
			Ok(post) => return Ok(post),
			Err(_) => return Err(Status::InternalServerError),
		};
	}

	let image = match post.image {
		Some(image) => image,
		None => return Err(Status::BadRequest),
	};
	let images_extra = match post.images_extra {
		Some(image) => image,
		None => return Err(Status::BadRequest),
	};
	let new_post = NewPost {
		post_name: &post.name,
		post_text: &post.text,
		post_text_short: &post.text_short,
		post_image: &image,
		post_images_extra: &images_extra,
		post_uploader: user.id,
		post_link: &post.link,
		post_game_tag: post.game_tag,
		post_type_tag: post.type_tag,
		post_downloads: 0,
	};

	let result = diesel::insert_into(posts::table)
		.values(&new_post)
		.get_result(conn);

	match result {
		Ok(post) => Ok(post),
		Err(_) => Err(Status::InternalServerError),
	}
}

pub fn update_post(conn: &mut PgConnection, post: PostMetadata, update_id: i32) -> Option<Post> {
	diesel::update(posts::table.filter(posts::post_id.eq(update_id)))
		.set((
			posts::post_name.eq(&post.name),
			posts::post_text.eq(&post.text),
			posts::post_text_short.eq(&post.text_short),
			posts::post_game_tag.eq(post.game_tag),
			posts::post_type_tag.eq(post.type_tag),
		))
		.get_result::<Post>(conn)
		.ok()
}

pub fn has_liked_post(conn: &mut PgConnection, user_id: i64, post_id: i32) -> bool {
	users_liked_posts::table
		.filter(users_liked_posts::user_id.eq(user_id))
		.filter(users_liked_posts::post_id.eq(post_id))
		.get_result::<LikedPost>(conn)
		.is_ok()
}

pub fn like_post_from_ids(
	conn: &mut PgConnection,
	user_id: i64,
	post_id: i32,
) -> Option<LikedPost> {
	let new_like = NewLikedPost { post_id, user_id };

	let has_liked = has_liked_post(conn, user_id, post_id);

	if has_liked {
		let _result = diesel::delete(
			users_liked_posts::table
				.filter(users_liked_posts::user_id.eq(user_id))
				.filter(users_liked_posts::post_id.eq(post_id)),
		)
		.get_result::<LikedPost>(conn);
	}

	let has_disliked = users_disliked_posts::table
		.filter(users_disliked_posts::user_id.eq(user_id))
		.filter(users_disliked_posts::post_id.eq(post_id))
		.get_result::<DislikedPost>(conn)
		.is_ok();

	if has_disliked {
		let _result = diesel::delete(users_disliked_posts::table)
			.filter(users_disliked_posts::user_id.eq(user_id))
			.filter(users_disliked_posts::post_id.eq(post_id))
			.get_result::<DislikedPost>(conn);
	}

	diesel::insert_into(users_liked_posts::table)
		.values(&new_like)
		.get_result::<LikedPost>(conn)
		.ok()
}

pub fn has_disliked_post(conn: &mut PgConnection, user_id: i64, post_id: i32) -> bool {
	users_disliked_posts::table
		.filter(users_disliked_posts::user_id.eq(user_id))
		.filter(users_disliked_posts::post_id.eq(post_id))
		.get_result::<DislikedPost>(conn)
		.is_ok()
}

pub fn dislike_post_from_ids(
	conn: &mut PgConnection,
	user_id: i64,
	post_id: i32,
) -> Option<DislikedPost> {
	let new_like = NewDislikedPost { post_id, user_id };

	let has_disliked = has_disliked_post(conn, user_id, post_id);

	if has_disliked {
		let _result = diesel::delete(
			users_disliked_posts::table
				.filter(users_disliked_posts::user_id.eq(user_id))
				.filter(users_disliked_posts::post_id.eq(post_id)),
		)
		.get_result::<DislikedPost>(conn);
	}

	let has_liked = users_liked_posts::table
		.filter(users_liked_posts::user_id.eq(user_id))
		.filter(users_liked_posts::post_id.eq(post_id))
		.get_result::<LikedPost>(conn)
		.is_ok();

	if has_liked {
		let _result = diesel::delete(users_liked_posts::table)
			.filter(users_liked_posts::user_id.eq(user_id))
			.filter(users_liked_posts::post_id.eq(post_id))
			.get_result::<LikedPost>(conn);
	}

	diesel::insert_into(users_disliked_posts::table)
		.values(&new_like)
		.get_result::<DislikedPost>(conn)
		.ok()
}

pub fn get_additional_post_data(
	connection: &mut PgConnection,
	post: DetailedPostNoDepends,
) -> DetailedPost {
	let dependencies = post_dependencies::table
		.filter(post_dependencies::post_id.eq(post.id))
		.inner_join(posts::table.on(posts::post_id.eq(post_dependencies::dependency_id)))
		.inner_join(users::table.on(users::user_id.eq(posts::post_uploader)))
		.left_join(
			users_liked_posts::table
				.on(users_liked_posts::post_id.eq(post_dependencies::dependency_id)),
		)
		.left_join(
			users_disliked_posts::table
				.on(users_disliked_posts::post_id.eq(post_dependencies::dependency_id)),
		)
		.group_by((posts::post_id, users::user_id))
		.select((
			posts::post_id,
			posts::post_name,
			posts::post_text,
			posts::post_text_short,
			posts::post_image,
			posts::post_images_extra,
			posts::post_link,
			posts::post_date,
			posts::post_game_tag,
			posts::post_type_tag,
			count_distinct(users_liked_posts::user_id.nullable()),
			count_distinct(users_disliked_posts::user_id.nullable()),
			posts::post_downloads,
			(users::user_id, users::user_name, users::user_avatar),
		))
		.load::<DetailedPostNoDepends>(connection)
		.unwrap_or_else(|_| vec![]);

	let changelogs = post_changelogs::table
		.filter(post_changelogs::post_id.eq(post.id))
		.order(post_changelogs::time.desc())
		.select((
			post_changelogs::description,
			post_changelogs::time,
			post_changelogs::download,
		))
		.load::<Changelog>(connection)
		.unwrap_or_else(|_| vec![]);

	let comments = post_comments::table
		.filter(post_comments::post_id.eq(post.id))
		.order(post_comments::comment_date.desc())
		.inner_join(users::table.on(users::user_id.eq(post_comments::user_id)))
		.select((
			post_comments::comment_id,
			(users::user_id, users::user_name, users::user_avatar),
			post_comments::comment_text,
			post_comments::comment_parent,
			post_comments::comment_date,
		))
		.load::<Comment>(connection)
		.unwrap_or_else(|_| vec![]);

	DetailedPost {
		id: post.id,
		name: post.name,
		text: post.text,
		text_short: post.text_short,
		image: post.image,
		images_extra: post.images_extra,
		link: post.link,
		date: post.date,
		game_tag: post.game_tag,
		type_tag: post.type_tag,
		likes: post.likes,
		dislikes: post.dislikes,
		downloads: post.downloads,
		user: post.user,
		dependencies,
		changelogs,
		comments,
	}
}

pub fn get_additional_posts_data(
	connection: &mut PgConnection,
	posts: Vec<DetailedPostNoDepends>,
) -> Vec<DetailedPost> {
	let mut results = vec![];
	for post in posts {
		results.push(get_additional_post_data(connection, post));
	}
	results
}

macro_rules! detailed_post_base {
	() => {
		posts::table
			.inner_join(users::table)
			.left_join(users_liked_posts::table)
			.left_join(users_disliked_posts::table)
			.group_by((posts::post_id, users::user_id))
			.select((
				posts::post_id,
				posts::post_name,
				posts::post_text,
				posts::post_text_short,
				posts::post_image,
				posts::post_images_extra,
				posts::post_link,
				posts::post_date,
				posts::post_game_tag,
				posts::post_type_tag,
				count_distinct(users_liked_posts::user_id.nullable()),
				count_distinct(users_disliked_posts::user_id.nullable()),
				posts::post_downloads,
				(users::user_id, users::user_name, users::user_avatar),
			))
	};
	($limit:ident, $offset:ident) => {
		detailed_post_base!().limit($limit).offset($offset)
	};
}

macro_rules! short_post_base {
	() => {
		posts::table
			.left_join(users_liked_posts::table)
			.left_join(users_disliked_posts::table)
			.group_by(posts::post_id)
			.select((
				posts::post_id,
				posts::post_name,
				posts::post_text_short,
				posts::post_image,
				posts::post_game_tag,
				posts::post_type_tag,
				count_distinct(users_liked_posts::user_id.nullable()),
				count_distinct(users_disliked_posts::user_id.nullable()),
				posts::post_downloads,
			))
	};
	($limit:ident, $offset:ident) => {
		short_post_base!().limit($limit).offset($offset)
	};
}

macro_rules! short_user_post_base {
	() => {
		users::table
			.inner_join(posts::table)
			.left_join(users_liked_posts::table.on(users_liked_posts::post_id.eq(posts::post_id)))
			.left_join(
				users_disliked_posts::table.on(users_disliked_posts::post_id.eq(posts::post_id)),
			)
			.group_by((posts::post_id, users::user_id))
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
					posts::post_downloads,
				),
				(users::user_id, users::user_name, users::user_avatar),
			))
	};
	($limit:ident, $offset:ident) => {
		short_user_post_base!().limit($limit).offset($offset)
	};
}

pub fn get_latest_posts(
	connection: &mut PgConnection,
	name: String,
	offset: i64,
	game_tag: i32,
	limit: i64,
) -> Option<Vec<ShortPost>> {
	short_post_base!(limit, offset)
		.filter(posts::post_game_tag.eq(game_tag))
		.filter(posts::post_name.ilike(format!("%{}%", name)))
		.order(posts::post_date.desc())
		.load::<ShortPost>(connection)
		.ok()
		.filter(|posts| !posts.is_empty())
}

pub fn get_latest_posts_detailed(
	connection: &mut PgConnection,
	name: String,
	offset: i64,
	game_tag: i32,
	limit: i64,
) -> Option<Vec<DetailedPost>> {
	detailed_post_base!(limit, offset)
		.filter(posts::post_game_tag.eq(game_tag))
		.filter(posts::post_name.ilike(format!("%{}%", name)))
		.order(posts::post_date.desc())
		.load::<DetailedPostNoDepends>(connection)
		.ok()
		.filter(|posts| !posts.is_empty())
		.map(|posts| get_additional_posts_data(connection, posts))
}

pub fn get_latest_posts_disallowed(
	connection: &mut PgConnection,
	name: String,
	offset: i64,
	game_tag: i32,
	disallowed: Vec<i32>,
	limit: i64,
) -> Option<Vec<ShortPost>> {
	short_post_base!(limit, offset)
		.filter(posts::post_name.ilike(format!("%{}%", name)))
		.filter(posts::post_id.ne_all(disallowed))
		.filter(posts::post_game_tag.eq(game_tag))
		.order(posts::post_date.desc())
		.load::<ShortPost>(connection)
		.ok()
		.filter(|posts| !posts.is_empty())
}

pub fn get_latest_posts_unfiltered(
	connection: &mut PgConnection,
	limit: i64,
) -> Option<Vec<ShortPost>> {
	short_post_base!()
		.order(posts::post_date.desc())
		.limit(limit)
		.load::<ShortPost>(connection)
		.ok()
		.filter(|posts| !posts.is_empty())
}

pub fn get_popular_posts(
	connection: &mut PgConnection,
	name: String,
	offset: i64,
	game_tag: i32,
	limit: i64,
) -> Option<Vec<ShortPost>> {
	short_post_base!(limit, offset)
		.filter(posts::post_name.ilike(format!("%{}%", name)))
		.filter(posts::post_game_tag.eq(game_tag))
		.order(posts::post_downloads.desc())
		.load::<ShortPost>(connection)
		.ok()
		.filter(|posts| !posts.is_empty())
}

pub fn get_popular_posts_detailed(
	connection: &mut PgConnection,
	name: String,
	offset: i64,
	game_tag: i32,
	limit: i64,
) -> Option<Vec<DetailedPost>> {
	detailed_post_base!(limit, offset)
		.filter(posts::post_game_tag.eq(game_tag))
		.filter(posts::post_name.ilike(format!("%{}%", name)))
		.order(posts::post_downloads.desc())
		.load::<DetailedPostNoDepends>(connection)
		.ok()
		.filter(|posts| !posts.is_empty())
		.map(|posts| get_additional_posts_data(connection, posts))
}

pub fn get_popular_posts_disallowed(
	connection: &mut PgConnection,
	name: String,
	offset: i64,
	game_tag: i32,
	disallowed: Vec<i32>,
	limit: i64,
) -> Option<Vec<ShortPost>> {
	short_post_base!(limit, offset)
		.filter(posts::post_name.ilike(format!("%{}%", name)))
		.filter(posts::post_id.ne_all(disallowed))
		.filter(posts::post_game_tag.eq(game_tag))
		.order(posts::post_downloads.desc())
		.load::<ShortPost>(connection)
		.ok()
		.filter(|posts| !posts.is_empty())
}

pub fn get_post(connection: &mut PgConnection, id: i32) -> Option<DetailedPost> {
	detailed_post_base!()
		.filter(posts::post_id.eq(id))
		.first::<DetailedPostNoDepends>(connection)
		.ok()
		.map(|post| get_additional_post_data(connection, post))
}

pub fn get_short_post(conn: &mut PgConnection, id: i32) -> Option<ShortPost> {
	short_post_base!()
		.filter(posts::post_id.eq(id))
		.first::<ShortPost>(conn)
		.ok()
}

pub fn get_posts_detailed(
	connection: &mut PgConnection,
	ids: Vec<i32>,
) -> Option<Vec<DetailedPost>> {
	detailed_post_base!()
		.filter(posts::post_id.eq_any(ids))
		.order(posts::post_id.asc())
		.load::<DetailedPostNoDepends>(connection)
		.ok()
		.filter(|posts| !posts.is_empty())
		.map(|posts| get_additional_posts_data(connection, posts))
}

pub fn get_changed_posts_detailed(
	connection: &mut PgConnection,
	since: time::PrimitiveDateTime,
) -> Option<Vec<DetailedPost>> {
	detailed_post_base!()
		.filter(posts::post_date.gt(since))
		.order(posts::post_date.desc())
		.load::<DetailedPostNoDepends>(connection)
		.ok()
		.filter(|posts| !posts.is_empty())
		.map(|posts| get_additional_posts_data(connection, posts))
}

pub fn get_changed_posts_short(
	connection: &mut PgConnection,
	since: time::PrimitiveDateTime,
) -> Option<Vec<ShortPost>> {
	short_post_base!()
		.filter(posts::post_date.gt(since))
		.order(posts::post_date.desc())
		.load::<ShortPost>(connection)
		.ok()
		.filter(|posts| !posts.is_empty())
}

pub fn get_user_posts_latest(
	conn: &mut PgConnection,
	id: i64,
	offset: i64,
	game_tag: i32,
	limit: i64,
) -> Option<Vec<ShortUserPosts>> {
	short_user_post_base!(limit, offset)
		.filter(users::user_id.eq(id))
		.filter(posts::post_game_tag.eq(game_tag))
		.order(posts::post_date.desc())
		.load::<ShortUserPosts>(conn)
		.ok()
		.filter(|posts| !posts.is_empty())
}

pub fn get_user_posts_popular(
	conn: &mut PgConnection,
	id: i64,
	offset: i64,
	game_tag: i32,
	limit: i64,
) -> Option<Vec<ShortUserPosts>> {
	short_user_post_base!(limit, offset)
		.filter(users::user_id.eq(id))
		.filter(posts::post_game_tag.eq(game_tag))
		.order(posts::post_downloads.desc())
		.load::<ShortUserPosts>(conn)
		.ok()
		.filter(|posts| !posts.is_empty())
}

pub fn delete_post(conn: &mut PgConnection, id: i32) -> bool {
	diesel::delete(posts::table.filter(posts::post_id.eq(id)))
		.execute(conn)
		.is_ok()
}

pub fn update_download_count(conn: &mut PgConnection, path: String) -> Status {
	let result = posts::table
		.filter(posts::post_link.eq(path))
		.select(posts::post_id)
		.first::<i32>(conn);

	let post_id = match result {
		Ok(post_id) => post_id,
		Err(_) => return Status::NotFound,
	};
	let result = diesel::update(posts::table)
		.filter(posts::post_id.eq(post_id))
		.set(posts::post_downloads.eq(posts::post_downloads + 1))
		.execute(conn);
	if result.is_ok() {
		Status::Ok
	} else {
		Status::InternalServerError
	}
}

pub fn owns_post(conn: &mut PgConnection, id: i32, user_id: i64) -> bool {
	posts::table
		.filter(posts::post_uploader.eq(user_id))
		.filter(posts::post_id.eq(id))
		.first::<Post>(conn)
		.is_ok()
}

pub fn add_dependency(conn: &mut PgConnection, post_id: i32, dependency_id: i32) -> bool {
	diesel::insert_into(post_dependencies::table)
		.values((
			post_dependencies::post_id.eq(post_id),
			post_dependencies::dependency_id.eq(dependency_id),
		))
		.execute(conn)
		.is_ok()
}

pub fn remove_dependency(conn: &mut PgConnection, post_id: i32, dependency_id: i32) -> bool {
	diesel::delete(
		post_dependencies::table
			.filter(post_dependencies::post_id.eq(post_id))
			.filter(post_dependencies::dependency_id.eq(dependency_id)),
	)
	.execute(conn)
	.is_ok()
}

pub fn get_reports(conn: &mut PgConnection) -> Option<Vec<Report>> {
	reports::table
		.inner_join(posts::table.on(posts::post_id.eq(reports::post_id)))
		.inner_join(users::table.on(users::user_id.eq(reports::user_id)))
		.left_join(users_liked_posts::table.on(users_liked_posts::post_id.eq(reports::post_id)))
		.left_join(
			users_disliked_posts::table.on(users_disliked_posts::post_id.eq(reports::post_id)),
		)
		.order(reports::time.desc())
		.group_by((posts::post_id, users::user_id, reports::report_id))
		.select((
			reports::report_id,
			(users::user_id, users::user_name, users::user_avatar),
			(
				posts::post_id,
				posts::post_name,
				posts::post_text_short,
				posts::post_image,
				posts::post_game_tag,
				posts::post_type_tag,
				count_distinct(users_liked_posts::user_id.nullable()),
				count_distinct(users_disliked_posts::user_id.nullable()),
				posts::post_downloads,
			),
			reports::description,
		))
		.load::<Report>(conn)
		.ok()
}

pub fn delete_report(conn: &mut PgConnection, id: i32) -> bool {
	diesel::delete(reports::table.filter(reports::report_id.eq(id)))
		.execute(conn)
		.is_ok()
}

pub fn add_report(conn: &mut PgConnection, post_id: i32, user_id: i64, reason: String) -> bool {
	diesel::insert_into(reports::table)
		.values((
			reports::post_id.eq(post_id),
			reports::user_id.eq(user_id),
			reports::time.eq(chrono::NaiveDateTime::from_timestamp(
				chrono::Utc::now().timestamp(),
				0,
			)),
			reports::description.eq(reason),
		))
		.execute(conn)
		.is_ok()
}

pub fn get_post_count(conn: &mut PgConnection, name: String, game_tag: i32) -> Option<i64> {
	posts::table
		.filter(posts::post_name.ilike(format!("%{}%", name)))
		.filter(posts::post_game_tag.eq(game_tag))
		.count()
		.get_result(conn)
		.ok()
}

pub fn get_post_ids(conn: &mut PgConnection) -> Option<Vec<SitemapInfo>> {
	posts::table
		.select((posts::post_id, posts::post_date))
		.load::<SitemapInfo>(conn)
		.ok()
}

pub fn get_post_latest_date(conn: &mut PgConnection) -> Option<chrono::NaiveDateTime> {
	posts::table
		.select(posts::post_date)
		.order(posts::post_date.desc())
		.first(conn)
		.ok()
}

pub fn add_changelog(
	connection: &mut PgConnection,
	id: i32,
	change: String,
	change_download: Option<String>,
) -> bool {
	diesel::insert_into(post_changelogs::table)
		.values((
			post_changelogs::post_id.eq(id),
			post_changelogs::description.eq(change),
			post_changelogs::time.eq(chrono::NaiveDateTime::from_timestamp(
				chrono::Utc::now().timestamp(),
				0,
			)),
			post_changelogs::download.eq(change_download),
		))
		.execute(connection)
		.is_ok()
}

pub fn add_comment(
	connection: &mut PgConnection,
	user: i64,
	post: i32,
	comment: String,
	parent: Option<i32>,
) -> bool {
	diesel::insert_into(post_comments::table)
		.values((
			post_comments::user_id.eq(user),
			post_comments::post_id.eq(post),
			post_comments::comment_text.eq(comment),
			post_comments::comment_date.eq(chrono::NaiveDateTime::from_timestamp(
				chrono::Utc::now().timestamp(),
				0,
			)),
			post_comments::comment_parent.eq(parent),
		))
		.execute(connection)
		.is_ok()
}

pub fn delete_comment(connection: &mut PgConnection, id: i32, user: i64) -> bool {
	diesel::delete(
		post_comments::table
			.filter(post_comments::comment_id.eq(id))
			.filter(post_comments::user_id.eq(user)),
	)
	.execute(connection)
	.is_ok()
}

pub fn update_download_limit(connection: &mut PgConnection, ip: IpAddr, size: i64) -> Status {
	let current_time = chrono::Utc::now().date_naive().and_hms(0, 0, 0);
	let limit_exists = download_limit::table
		.filter(download_limit::ip.eq(ip.to_string()))
		.filter(download_limit::date.eq(current_time))
		.select(download_limit::downloaded)
		.get_result::<i64>(connection);
	if let Ok(used_limit) = limit_exists {
		if used_limit >= 3 * 1024 * 1024 * 1024 {
			return Status::TooManyRequests;
		}
		let _ = diesel::update(
			download_limit::table
				.filter(download_limit::ip.eq(ip.to_string()))
				.filter(download_limit::date.eq(current_time)),
		)
		.set((
			download_limit::date.eq(current_time),
			download_limit::ip.eq(ip.to_string()),
			download_limit::downloaded.eq(used_limit + size),
		))
		.execute(connection);
	} else {
		let _ = diesel::insert_into(download_limit::table)
			.values((
				download_limit::date.eq(current_time),
				download_limit::ip.eq(ip.to_string()),
				download_limit::downloaded.eq(size),
			))
			.execute(connection);
	}

	Status::Ok
}

pub fn get_update_dates(
	connection: &mut PgConnection,
	ids: Vec<i32>,
) -> Option<Vec<PostUpdateTime>> {
	posts::table
		.filter(posts::post_id.eq_any(ids))
		.order(posts::post_id.asc())
		.select((posts::post_id, posts::post_date))
		.load::<PostUpdateTime>(connection)
		.ok()
}

pub fn create_user<'a>(
	conn: &mut PgConnection,
	id: i64,
	name: &'a str,
	avatar: &'a str,
) -> Option<User> {
	// Check if entry with same user id already exists, if so update name and avatar
	let user = users::table
		.filter(users::user_id.eq(id))
		.first::<User>(conn);
	if user.is_ok() {
		let result = diesel::update(users::table.filter(users::user_id.eq(id)))
			.set((users::user_name.eq(name), users::user_avatar.eq(avatar)))
			.get_result(conn);
		match result {
			Ok(user) => Some(user),
			Err(_) => None,
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
			Ok(user) => Some(user),
			Err(_) => None,
		}
	}
}

// Ensure the user is verified before calling this
pub fn delete_user(conn: &mut PgConnection, id: i64) -> bool {
	diesel::delete(users::table.filter(users::user_id.eq(id)))
		.execute(conn)
		.is_ok()
}

pub fn get_user(conn: &mut PgConnection, id: i64) -> Option<User> {
	users::table
		.filter(users::user_id.eq(id))
		.get_result(conn)
		.ok()
}

pub fn get_user_stats(connection: &mut PgConnection, id: i64) -> UserStats {
	let downloads = posts::table
		.filter(posts::post_uploader.eq(id))
		.select(sum(posts::post_downloads))
		.get_result::<Option<bigdecimal::BigDecimal>>(connection)
		.unwrap_or_default()
		.unwrap_or_default()
		.to_i64()
		.unwrap_or_default();
	let likes = users_liked_posts::table
		.inner_join(posts::table)
		.filter(posts::post_uploader.eq(id))
		.select(count(users_liked_posts::user_id))
		.get_result::<i64>(connection)
		.unwrap_or_default();
	let dislikes = users_disliked_posts::table
		.inner_join(posts::table)
		.filter(posts::post_uploader.eq(id))
		.select(count(users_disliked_posts::user_id))
		.get_result::<i64>(connection)
		.unwrap_or_default();

	UserStats {
		likes,
		dislikes,
		downloads,
	}
}

pub fn get_user_liked_posts(
	conn: &mut PgConnection,
	id: i64,
	offset: i64,
	limit: i64,
) -> Option<Vec<ShortPostNoLikes>> {
	users_liked_posts::table
		.filter(users_liked_posts::user_id.eq(id))
		.inner_join(posts::table)
		.group_by(posts::post_id)
		.order(posts::post_date.desc())
		.select((
			posts::post_id,
			posts::post_name,
			posts::post_text_short,
			posts::post_image,
			posts::post_game_tag,
			posts::post_type_tag,
			posts::post_downloads,
		))
		.limit(limit)
		.offset(offset)
		.load::<ShortPostNoLikes>(conn)
		.ok()
}
