diesel::table! {
	download_limit (limit_id) {
		limit_id -> Int4,
		ip -> Text,
		downloaded -> Int8,
		date -> Timestamp,
	}
}

diesel::table! {
	post_changelogs (changelog_id) {
		changelog_id -> Int4,
		post_id -> Int4,
		description -> Text,
		time -> Timestamp,
		download -> Nullable<Text>,
	}
}

diesel::table! {
	post_comments (comment_id) {
		comment_id -> Int4,
		post_id -> Int4,
		user_id -> Int8,
		comment_text -> Text,
		comment_parent -> Nullable<Int4>,
		comment_date -> Timestamp,
	}
}

diesel::table! {
	post_dependencies (post_id, dependency_id) {
		post_id -> Int4,
		dependency_id -> Int4,
	}
}

diesel::table! {
	posts (post_id) {
		post_id -> Int4,
		post_name -> Text,
		post_text -> Text,
		post_text_short -> Text,
		post_image -> Text,
		post_images_extra -> Array<Text>,
		post_uploader -> Int8,
		post_link -> Text,
		post_date -> Timestamp,
		post_game_tag -> Int4,
		post_type_tag -> Int4,
		post_downloads -> Int8,
	}
}

diesel::table! {
	reports (report_id) {
		report_id -> Int4,
		user_id -> Int8,
		post_id -> Int4,
		description -> Text,
		time -> Timestamp,
	}
}

diesel::table! {
	users (user_id) {
		user_id -> Int8,
		user_name -> Text,
		user_avatar -> Text,
	}
}

diesel::table! {
	users_disliked_posts (disliked_id) {
		disliked_id -> Int4,
		post_id -> Int4,
		user_id -> Int8,
	}
}

diesel::table! {
	users_liked_posts (liked_id) {
		liked_id -> Int4,
		post_id -> Int4,
		user_id -> Int8,
	}
}

diesel::joinable!(post_changelogs -> posts (post_id));
diesel::joinable!(post_comments -> posts (post_id));
diesel::joinable!(post_comments -> users (user_id));
diesel::joinable!(posts -> users (post_uploader));
diesel::joinable!(reports -> posts (post_id));
diesel::joinable!(reports -> users (user_id));
diesel::joinable!(users_disliked_posts -> posts (post_id));
diesel::joinable!(users_disliked_posts -> users (user_id));
diesel::joinable!(users_liked_posts -> posts (post_id));
diesel::joinable!(users_liked_posts -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
	download_limit,
	post_changelogs,
	post_comments,
	post_dependencies,
	posts,
	reports,
	users,
	users_disliked_posts,
	users_liked_posts,
);

diesel::allow_columns_to_appear_in_same_group_by_clause!(
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
	posts::post_downloads,
	users::user_id,
	users::user_name,
	users::user_avatar,
	reports::report_id,
	reports::user_id,
	reports::post_id,
	reports::description,
	reports::time,
	post_comments::comment_id,
	post_comments::comment_text,
	post_comments::comment_parent,
	post_comments::comment_date,
);
