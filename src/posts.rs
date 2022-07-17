use super::models::*;
use super::schema::*;
use diesel::dsl::*;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use rocket::http::Status;

pub fn create_post(
	conn: &mut PgConnection,
	post: PostUnidentified,
	user: User,
	update_id: i32,
) -> Result<Post, Status> {
	if update_id != -1 {
		let result = diesel::update(posts::table.filter(posts::post_id.eq(update_id)))
			.set((
				posts::post_name.eq(&post.name),
				posts::post_text.eq(&post.text),
				posts::post_text_short.eq(&post.text_short),
				posts::post_image.eq(&post.image),
				posts::post_link.eq(&post.link),
			))
			.get_result::<Post>(conn);

		if result.is_ok() {
			return Ok(result.unwrap());
		} else {
			return Err(Status::InternalServerError);
		}
	}

	let new_post = NewPost {
		post_name: &post.name,
		post_text: &post.text,
		post_text_short: &post.text_short,
		post_image: &post.image,
		post_uploader: user.id,
		post_link: &post.link,
	};

	let result = diesel::insert_into(posts::table)
		.values(&new_post)
		.get_result(conn);

	if result.is_ok() {
		Ok(result.unwrap())
	} else {
		Err(Status::InternalServerError)
	}
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
) -> Result<LikedPost, Status> {
	let new_like = NewLikedPost { post_id, user_id };

	let has_liked = has_liked_post(conn, user_id, post_id);

	if has_liked {
		return Err(Status::Conflict);
	}

	let has_disliked = users_disliked_posts::table
		.filter(users_disliked_posts::user_id.eq(user_id))
		.filter(users_disliked_posts::post_id.eq(post_id))
		.get_result::<DislikedPost>(conn)
		.is_ok();

	if has_disliked {
		let _ = diesel::delete(users_disliked_posts::table)
			.filter(users_disliked_posts::user_id.eq(user_id))
			.filter(users_disliked_posts::post_id.eq(post_id))
			.get_result::<DislikedPost>(conn);
	}

	let result = diesel::insert_into(users_liked_posts::table)
		.values(&new_like)
		.get_result::<LikedPost>(conn)
		.unwrap_or(LikedPost {
			id: -1,
			post: -1,
			user: -1,
		});

	if result.id != -1 {
		Ok(result)
	} else {
		Err(Status::InternalServerError)
	}
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
) -> Result<DislikedPost, Status> {
	let new_like = NewDislikedPost { post_id, user_id };

	let has_disliked = has_disliked_post(conn, user_id, post_id);

	if has_disliked {
		return Err(Status::Conflict);
	}

	let has_liked = users_liked_posts::table
		.filter(users_liked_posts::user_id.eq(user_id))
		.filter(users_liked_posts::post_id.eq(post_id))
		.get_result::<LikedPost>(conn)
		.is_ok();

	if has_liked {
		let _ = diesel::delete(users_liked_posts::table)
			.filter(users_liked_posts::user_id.eq(user_id))
			.filter(users_liked_posts::post_id.eq(post_id))
			.get_result::<LikedPost>(conn);
	}

	let result = diesel::insert_into(users_disliked_posts::table)
		.values(&new_like)
		.get_result::<DislikedPost>(conn)
		.unwrap_or(DislikedPost {
			id: -1,
			post: -1,
			user: -1,
		});

	if result.id != -1 {
		Ok(result)
	} else {
		Err(Status::InternalServerError)
	}
}

pub fn get_latest_posts(
	connection: &mut PgConnection,
	name: String,
	offset: i64,
) -> Result<Vec<ShortPost>, Status> {
	let results = posts::table
		.left_join(users_liked_posts::table)
		.left_join(users_disliked_posts::table)
		.left_join(download_stats::table.on(download_stats::post_id.eq(posts::post_id)))
		.group_by(posts::post_id)
		.filter(posts::post_name.ilike(format!("%{}%", name)))
		.order_by(posts::post_id.desc())
		.select((
			posts::post_id,
			posts::post_name,
			posts::post_text_short,
			posts::post_image,
			count_distinct(users_liked_posts::user_id.nullable()),
			count_distinct(users_disliked_posts::user_id.nullable()),
			count_distinct(download_stats::timestamp.nullable()),
		))
		.limit(30)
		.offset(offset)
		.load::<(i32, String, String, String, i64, i64, i64)>(connection)
		.unwrap_or_else(|_| vec![]);

	if results.is_empty() {
		return Err(Status::NotFound);
	}
	Ok(results
		.iter()
		.map(|post| ShortPost {
			id: post.0,
			name: post.1.clone(),
			text_short: post.2.clone(),
			image: post.3.clone(),
			likes: post.4,
			dislikes: post.5,
			downloads: post.6,
		})
		.collect::<Vec<ShortPost>>())
}

pub fn get_latest_posts_detailed(
	connection: &mut PgConnection,
	name: String,
	offset: i64,
) -> Result<Vec<DetailedPost>, Status> {
	let results = posts::table
		.filter(posts::post_name.ilike(format!("%{}%", name)))
		.inner_join(users::table)
		.left_join(users_liked_posts::table)
		.left_join(users_disliked_posts::table)
		.left_join(download_stats::table.on(download_stats::post_id.eq(posts::post_id)))
		.group_by((posts::post_id, users::user_id))
		.order_by(posts::post_id.desc())
		.select((
			posts::post_id,
			posts::post_name,
			posts::post_text,
			posts::post_text_short,
			posts::post_image,
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
			String,
			i64,
			i64,
			i64,
			i64,
			String,
			String,
		)>(connection)
		.unwrap_or_else(|_| vec![]);

	if results.is_empty() {
		return Err(Status::NotFound);
	}
	let mut posts = vec![];
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
				String,
				i64,
				i64,
				i64,
				i64,
				String,
				String,
			)>(connection)
			.unwrap_or_else(|_| vec![]);

		posts.push(DetailedPost {
			id: post.0,
			name: post.1.clone(),
			text: post.2.clone(),
			text_short: post.3.clone(),
			image: post.4.clone(),
			link: post.5.clone(),
			likes: post.6,
			dislikes: post.7,
			downloads: post.8,
			user: User {
				id: post.9,
				name: post.10.clone(),
				avatar: post.11.clone(),
			},
			dependencies: dependencies
				.into_iter()
				.map(|dependency| DetailedPost {
					id: dependency.0,
					name: dependency.1.clone(),
					text: dependency.2.clone(),
					text_short: dependency.3.clone(),
					image: dependency.4.clone(),
					link: dependency.5.clone(),
					likes: dependency.6,
					dislikes: dependency.7,
					downloads: dependency.8,
					user: User {
						id: dependency.9,
						name: dependency.10.clone(),
						avatar: dependency.11.clone(),
					},
					dependencies: vec![],
				})
				.collect(),
		});
	}
	Ok(posts)
}

pub fn get_latest_posts_disallowed(
	connection: &mut PgConnection,
	name: String,
	offset: i64,
	disallowed: Vec<i32>,
) -> Result<Vec<ShortPost>, Status> {
	let results = posts::table
		.left_join(users_liked_posts::table)
		.left_join(users_disliked_posts::table)
		.left_join(download_stats::table.on(download_stats::post_id.eq(posts::post_id)))
		.group_by(posts::post_id)
		.filter(posts::post_name.ilike(format!("%{}%", name)))
		.filter(posts::post_id.ne_all(disallowed))
		.order_by(posts::post_id.desc())
		.select((
			posts::post_id,
			posts::post_name,
			posts::post_text_short,
			posts::post_image,
			count_distinct(users_liked_posts::user_id.nullable()),
			count_distinct(users_disliked_posts::user_id.nullable()),
			count_distinct(download_stats::timestamp.nullable()),
		))
		.limit(30)
		.offset(offset)
		.load::<(i32, String, String, String, i64, i64, i64)>(connection)
		.unwrap_or_else(|_| vec![]);

	if results.is_empty() {
		return Err(Status::NotFound);
	}
	Ok(results
		.iter()
		.map(|post| ShortPost {
			id: post.0,
			name: post.1.clone(),
			text_short: post.2.clone(),
			image: post.3.clone(),
			likes: post.4,
			dislikes: post.5,
			downloads: post.6,
		})
		.collect::<Vec<ShortPost>>())
}

pub fn get_popular_posts(
	connection: &mut PgConnection,
	name: String,
	offset: i64,
) -> Result<Vec<ShortPost>, Status> {
	let results = posts::table
		.filter(posts::post_name.ilike(format!("%{}%", name)))
		.left_join(users_liked_posts::table)
		.left_join(users_disliked_posts::table)
		.left_join(download_stats::table.on(download_stats::post_id.eq(posts::post_id)))
		.group_by(posts::post_id)
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
		))
		.limit(30)
		.offset(offset)
		.load::<(i32, String, String, String, i64, i64, i64)>(connection)
		.unwrap_or_else(|_| vec![]);

	if results.is_empty() {
		return Err(Status::NotFound);
	}
	// For every post in results create ShortPost and push it to result vector
	Ok(results
		.iter()
		.map(|post| ShortPost {
			id: post.0,
			name: post.1.clone(),
			text_short: post.2.clone(),
			image: post.3.clone(),
			likes: post.4,
			dislikes: post.5,
			downloads: post.6,
		})
		.collect::<Vec<ShortPost>>())
}

pub fn get_popular_posts_detailed(
	connection: &mut PgConnection,
	name: String,
	offset: i64,
) -> Result<Vec<DetailedPost>, Status> {
	let results = posts::table
		.filter(posts::post_name.ilike(format!("%{}%", name)))
		.inner_join(users::table)
		.left_join(users_liked_posts::table)
		.left_join(users_disliked_posts::table)
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
			String,
			i64,
			i64,
			i64,
			i64,
			String,
			String,
		)>(connection)
		.unwrap_or_else(|_| vec![]);

	if results.is_empty() {
		return Err(Status::NotFound);
	}
	let mut posts = vec![];
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
				String,
				i64,
				i64,
				i64,
				i64,
				String,
				String,
			)>(connection)
			.unwrap_or_else(|_| vec![]);

		posts.push(DetailedPost {
			id: post.0,
			name: post.1.clone(),
			text: post.2.clone(),
			text_short: post.3.clone(),
			image: post.4.clone(),
			link: post.5.clone(),
			likes: post.6,
			dislikes: post.7,
			downloads: post.8,
			user: User {
				id: post.9,
				name: post.10.clone(),
				avatar: post.11.clone(),
			},
			dependencies: dependencies
				.into_iter()
				.map(|dependency| DetailedPost {
					id: dependency.0,
					name: dependency.1.clone(),
					text: dependency.2.clone(),
					text_short: dependency.3.clone(),
					image: dependency.4.clone(),
					link: dependency.5.clone(),
					likes: dependency.6,
					dislikes: dependency.7,
					downloads: dependency.8,
					user: User {
						id: dependency.9,
						name: dependency.10.clone(),
						avatar: dependency.11.clone(),
					},
					dependencies: vec![],
				})
				.collect(),
		});
	}
	Ok(posts)
}

pub fn get_popular_posts_disallowed(
	connection: &mut PgConnection,
	name: String,
	offset: i64,
	disallowed: Vec<i32>,
) -> Result<Vec<ShortPost>, Status> {
	let results = posts::table
		.left_join(users_liked_posts::table)
		.left_join(users_disliked_posts::table)
		.left_join(download_stats::table.on(download_stats::post_id.eq(posts::post_id)))
		.group_by(posts::post_id)
		.filter(posts::post_name.ilike(format!("%{}%", name)))
		.filter(posts::post_id.ne_all(disallowed))
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
		))
		.limit(30)
		.offset(offset)
		.load::<(i32, String, String, String, i64, i64, i64)>(connection)
		.unwrap_or_else(|_| vec![]);

	if results.is_empty() {
		return Err(Status::NotFound);
	}
	Ok(results
		.iter()
		.map(|post| ShortPost {
			id: post.0,
			name: post.1.clone(),
			text_short: post.2.clone(),
			image: post.3.clone(),
			likes: post.4,
			dislikes: post.5,
			downloads: post.6,
		})
		.collect::<Vec<ShortPost>>())
}

pub fn get_post(connection: &mut PgConnection, id: i32) -> Result<DetailedPost, Status> {
	let result = posts::table
		.filter(posts::post_id.eq(id))
		.inner_join(users::table)
		.left_join(users_liked_posts::table)
		.left_join(users_disliked_posts::table)
		.left_join(download_stats::table.on(download_stats::post_id.eq(posts::post_id)))
		.group_by((posts::post_id, users::user_id))
		.select((
			posts::post_id,
			posts::post_name,
			posts::post_text,
			posts::post_text_short,
			posts::post_image,
			posts::post_link,
			count_distinct(users_liked_posts::user_id.nullable()),
			count_distinct(users_disliked_posts::user_id.nullable()),
			count_distinct(download_stats::timestamp.nullable()),
			users::user_id,
			users::user_name,
			users::user_avatar,
		))
		.first::<(
			i32,
			String,
			String,
			String,
			String,
			String,
			i64,
			i64,
			i64,
			i64,
			String,
			String,
		)>(connection)
		.unwrap_or_else(|_| {
			(
				-1i32,
				String::new(),
				String::new(),
				String::new(),
				String::new(),
				String::new(),
				0i64,
				0i64,
				0i64,
				0i64,
				String::new(),
				String::new(),
			)
		});

	if result.0 == -1 {
		Err(Status::NotFound)
	} else {
		let dependencies = post_dependencies::table
			.filter(post_dependencies::post_id.eq(result.0))
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
				String,
				i64,
				i64,
				i64,
				i64,
				String,
				String,
			)>(connection)
			.unwrap_or_else(|_| vec![]);

		Ok(DetailedPost {
			id: result.0,
			name: result.1.clone(),
			text: result.2.clone(),
			text_short: result.3.clone(),
			image: result.4.clone(),
			link: result.5.clone(),
			likes: result.6,
			dislikes: result.7,
			downloads: result.8,
			user: User {
				id: result.9,
				name: result.10.clone(),
				avatar: result.11.clone(),
			},
			dependencies: dependencies
				.into_iter()
				.map(|dependency| DetailedPost {
					id: dependency.0,
					name: dependency.1.clone(),
					text: dependency.2.clone(),
					text_short: dependency.3.clone(),
					image: dependency.4.clone(),
					link: dependency.5.clone(),
					likes: dependency.6,
					dislikes: dependency.7,
					downloads: dependency.8,
					user: User {
						id: dependency.9,
						name: dependency.10.clone(),
						avatar: dependency.11.clone(),
					},
					dependencies: vec![],
				})
				.collect(),
		})
	}
}

pub fn delete_post(conn: &mut PgConnection, id: i32, user_id: i64) -> Status {
	let result = diesel::delete(
		posts::table
			.filter(posts::post_uploader.eq(user_id))
			.filter(posts::post_id.eq(id)),
	)
	.execute(conn);

	if result.is_ok() {
		Status::Ok
	} else {
		Status::NotFound
	}
}

pub fn update_download_count(conn: &mut PgConnection, path: String) -> Status {
	let result = posts::table
		.filter(posts::post_link.eq(path))
		.select(posts::post_id)
		.first::<i32>(conn);

	if result.is_ok() {
		let result = result.unwrap();
		let post_id = result;
		let result = diesel::insert_into(download_stats::table)
			.values(download_stats::post_id.eq(post_id))
			.execute(conn);
		if result.is_ok() {
			Status::Ok
		} else {
			Status::InternalServerError
		}
	} else {
		Status::NotFound
	}
}

pub fn owns_post(conn: &mut PgConnection, id: i32, user_id: i64) -> bool {
	let result = posts::table
		.filter(posts::post_uploader.eq(user_id))
		.filter(posts::post_id.eq(id))
		.first::<Post>(conn);

	result.is_ok()
}

pub fn add_dependency(conn: &mut PgConnection, post_id: i32, dependency_id: i32) -> Status {
	let result = diesel::insert_into(post_dependencies::table)
		.values((
			post_dependencies::post_id.eq(post_id),
			post_dependencies::dependency_id.eq(dependency_id),
		))
		.execute(conn);

	if result.is_ok() {
		Status::Ok
	} else {
		Status::InternalServerError
	}
}

pub fn remove_dependency(conn: &mut PgConnection, post_id: i32, dependency_id: i32) -> Status {
	let result = diesel::delete(
		post_dependencies::table
			.filter(post_dependencies::post_id.eq(post_id))
			.filter(post_dependencies::dependency_id.eq(dependency_id)),
	)
	.execute(conn);

	if result.is_ok() {
		Status::Ok
	} else {
		Status::InternalServerError
	}
}
