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
		if result.is_ok() {
			Ok(result.unwrap())
		} else {
			Err(Status::InternalServerError)
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
		if result.is_ok() {
			Ok(result.unwrap())
		} else {
			Err(Status::InternalServerError)
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

	if result.is_ok() {
		Ok(result.unwrap())
	} else {
		Err(Status::NotFound)
	}
}

pub fn get_user_posts_latest(
	conn: &mut PgConnection,
	id: i64,
	offset: i64,
) -> Result<UserPosts, Status> {
	let results = users::table
		.filter(users::user_id.eq(id))
		.inner_join(posts::table)
		.left_join(users_liked_posts::table.on(users_liked_posts::post_id.eq(posts::post_id)))
		.left_join(users_disliked_posts::table.on(users_disliked_posts::post_id.eq(posts::post_id)))
		.left_join(download_stats::table.on(download_stats::post_id.eq(posts::post_id)))
		.group_by((posts::post_id, users::user_id))
		.order_by(posts::post_id.desc())
		.select((
			posts::post_id,
			posts::post_name,
			posts::post_text_short,
			posts::post_image,
			count_distinct(users_liked_posts::user_id.nullable()),
			count_distinct(users_disliked_posts::user_id.nullable()),
			count_distinct(download_stats::timestamp.nullable()),
			users::user_id,
			users::user_name,
			users::user_avatar,
		))
		.limit(30)
		.offset(offset)
		.load::<(
			i32,
			String,
			String,
			String,
			i64,
			i64,
			i64,
			i64,
			String,
			String,
		)>(conn)
		.unwrap_or_else(|_| vec![]);

	if results.is_empty() {
		return Err(Status::NotFound);
	}
	let mut result = UserPosts {
		user: User {
			id: results[0].7,
			name: results[0].8.clone(),
			avatar: results[0].9.clone(),
		},
		posts: vec![],
	};
	for post in results {
		result.posts.push(ShortPost {
			id: post.0,
			name: post.1,
			text_short: post.2,
			image: post.3,
			likes: post.4,
			dislikes: post.5,
			downloads: post.6,
		});
	}
	Ok(result)
}

pub fn get_user_posts_latest_detailed(
	conn: &mut PgConnection,
	id: i64,
	offset: i64,
) -> Result<UserPostsDetailed, Status> {
	let results = users::table
		.filter(users::user_id.eq(id))
		.inner_join(posts::table)
		.left_join(users_liked_posts::table.on(users_liked_posts::post_id.eq(posts::post_id)))
		.left_join(users_disliked_posts::table.on(users_disliked_posts::post_id.eq(posts::post_id)))
		.left_join(download_stats::table.on(download_stats::post_id.eq(posts::post_id)))
		.group_by((posts::post_id, users::user_id))
		.order_by(posts::post_id.desc())
		.select((
			posts::post_id,
			posts::post_name,
			posts::post_text,
			posts::post_text_short,
			posts::post_image,
			posts::post_images_extra,
			posts::post_link,
			count_distinct(users_liked_posts::user_id.nullable()),
			count_distinct(users_disliked_posts::user_id.nullable()),
			count_distinct(download_stats::timestamp.nullable()),
			users::user_id,
			users::user_name,
			users::user_avatar,
		))
		.limit(30)
		.offset(offset)
		.load::<(
			i32,
			String,
			String,
			String,
			String,
			Vec<String>,
			String,
			i64,
			i64,
			i64,
			i64,
			String,
			String,
		)>(conn)
		.unwrap_or_else(|_| vec![]);

	if results.is_empty() {
		return Err(Status::NotFound);
	}
	let mut result = UserPostsDetailed {
		user: User {
			id: results[0].10,
			name: results[0].11.clone(),
			avatar: results[0].12.clone(),
		},
		posts: vec![],
	};
	for post in results {
		let dependencies = post_dependencies::table
			.filter(post_dependencies::post_id.eq(post.0))
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
			.left_join(
				download_stats::table
					.on(download_stats::post_id.eq(post_dependencies::dependency_id)),
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
				count_distinct(users_liked_posts::user_id.nullable()),
				count_distinct(users_disliked_posts::user_id.nullable()),
				count_distinct(download_stats::timestamp.nullable()),
				users::user_id,
				users::user_name,
				users::user_avatar,
			))
			.load::<(
				i32,
				String,
				String,
				String,
				String,
				Vec<String>,
				String,
				i64,
				i64,
				i64,
				i64,
				String,
				String,
			)>(conn)
			.unwrap_or_else(|_| vec![]);

		result.posts.push(DetailedPostNoUser {
			id: post.0,
			name: post.1,
			text: post.2,
			text_short: post.3,
			image: post.4,
			images_extra: post.5,
			link: post.6,
			likes: post.7,
			dislikes: post.8,
			downloads: post.9,
			dependencies: dependencies
				.into_iter()
				.map(|dependency| DetailedPostNoUser {
					id: dependency.0,
					name: dependency.1,
					text: dependency.2,
					text_short: dependency.3,
					image: dependency.4,
					images_extra: dependency.5,
					link: dependency.6,
					likes: dependency.7,
					dislikes: dependency.8,
					downloads: dependency.9,
					dependencies: vec![],
				})
				.collect(),
		});
	}
	Ok(result)
}

pub fn get_user_posts_popular(
	conn: &mut PgConnection,
	id: i64,
	offset: i64,
) -> Result<UserPosts, Status> {
	let results = users::table
		.filter(users::user_id.eq(id))
		.inner_join(posts::table)
		.left_join(users_liked_posts::table.on(users_liked_posts::post_id.eq(posts::post_id)))
		.left_join(users_disliked_posts::table.on(users_disliked_posts::post_id.eq(posts::post_id)))
		.left_join(download_stats::table.on(download_stats::post_id.eq(posts::post_id)))
		.group_by((posts::post_id, users::user_id))
		.order_by(
			(count_distinct(users_liked_posts::user_id.nullable())
				- count_distinct(users_disliked_posts::user_id.nullable()))
			.desc(),
		)
		.select((
			posts::post_id,
			posts::post_name,
			posts::post_text_short,
			posts::post_image,
			count_distinct(users_liked_posts::user_id.nullable()),
			count_distinct(users_disliked_posts::user_id.nullable()),
			count_distinct(download_stats::timestamp.nullable()),
			users::user_id,
			users::user_name,
			users::user_avatar,
		))
		.limit(30)
		.offset(offset)
		.load::<(
			i32,
			String,
			String,
			String,
			i64,
			i64,
			i64,
			i64,
			String,
			String,
		)>(conn)
		.unwrap_or_else(|_| vec![]);

	if results.is_empty() {
		return Err(Status::NotFound);
	}
	let mut result = UserPosts {
		user: User {
			id: results[0].7,
			name: results[0].8.clone(),
			avatar: results[0].9.clone(),
		},
		posts: vec![],
	};
	for post in results {
		result.posts.push(ShortPost {
			id: post.0,
			name: post.1,
			text_short: post.2,
			image: post.3,
			likes: post.4,
			dislikes: post.5,
			downloads: post.6,
		});
	}
	Ok(result)
}

pub fn get_user_posts_popular_detailed(
	conn: &mut PgConnection,
	id: i64,
	offset: i64,
) -> Result<UserPostsDetailed, Status> {
	let results = users::table
		.filter(users::user_id.eq(id))
		.inner_join(posts::table)
		.left_join(users_liked_posts::table.on(users_liked_posts::post_id.eq(posts::post_id)))
		.left_join(users_disliked_posts::table.on(users_disliked_posts::post_id.eq(posts::post_id)))
		.left_join(download_stats::table.on(download_stats::post_id.eq(posts::post_id)))
		.group_by((posts::post_id, users::user_id))
		.order_by(
			(count_distinct(users_liked_posts::user_id.nullable())
				- count_distinct(users_disliked_posts::user_id.nullable()))
			.desc(),
		)
		.select((
			posts::post_id,
			posts::post_name,
			posts::post_text,
			posts::post_text_short,
			posts::post_image,
			posts::post_images_extra,
			posts::post_link,
			count_distinct(users_liked_posts::user_id.nullable()),
			count_distinct(users_disliked_posts::user_id.nullable()),
			count_distinct(download_stats::timestamp.nullable()),
			users::user_id,
			users::user_name,
			users::user_avatar,
		))
		.limit(30)
		.offset(offset)
		.load::<(
			i32,
			String,
			String,
			String,
			String,
			Vec<String>,
			String,
			i64,
			i64,
			i64,
			i64,
			String,
			String,
		)>(conn)
		.unwrap_or_else(|_| vec![]);

	if results.is_empty() {
		return Err(Status::NotFound);
	}

	let mut result = UserPostsDetailed {
		user: User {
			id: results[0].10,
			name: results[0].11.clone(),
			avatar: results[0].12.clone(),
		},
		posts: vec![],
	};
	for post in results {
		let dependencies = post_dependencies::table
			.filter(post_dependencies::post_id.eq(post.0))
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
			.left_join(
				download_stats::table
					.on(download_stats::post_id.eq(post_dependencies::dependency_id)),
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
				count_distinct(users_liked_posts::user_id.nullable()),
				count_distinct(users_disliked_posts::user_id.nullable()),
				count_distinct(download_stats::timestamp.nullable()),
				users::user_id,
				users::user_name,
				users::user_avatar,
			))
			.load::<(
				i32,
				String,
				String,
				String,
				String,
				Vec<String>,
				String,
				i64,
				i64,
				i64,
				i64,
				String,
				String,
			)>(conn)
			.unwrap_or_else(|_| vec![]);

		result.posts.push(DetailedPostNoUser {
			id: post.0,
			name: post.1,
			text: post.2,
			text_short: post.3,
			image: post.4,
			images_extra: post.5,
			link: post.6,
			likes: post.7,
			dislikes: post.8,
			downloads: post.9,
			dependencies: dependencies
				.into_iter()
				.map(|dependency| DetailedPostNoUser {
					id: dependency.0,
					name: dependency.1,
					text: dependency.2,
					text_short: dependency.3,
					image: dependency.4,
					images_extra: dependency.5,
					link: dependency.6,
					likes: dependency.7,
					dislikes: dependency.8,
					downloads: dependency.9,
					dependencies: vec![],
				})
				.collect(),
		});
	}
	Ok(result)
}

pub fn get_user_stats(conn: &mut PgConnection, id: i64) -> (i64, i64, i64) {
	let results = users::table
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
		.get_result::<(i64, i64, i64)>(conn)
		.unwrap_or_else(|_| (0, 0, 0));
	(results.0, results.1, results.2)
}
