table! {
    download_stats (download_id) {
        download_id -> Int4,
        post_id -> Int4,
        timestamp -> Timestamp,
    }
}

table! {
    post_dependencies (post_id, dependency_id) {
        post_id -> Int4,
        dependency_id -> Int4,
    }
}

table! {
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
    }
}

table! {
    reports (report_id) {
        report_id -> Int4,
        user_id -> Int8,
        post_id -> Int4,
        description -> Text,
        time -> Timestamp,
    }
}

table! {
    users (user_id) {
        user_id -> Int8,
        user_name -> Text,
        user_avatar -> Text,
    }
}

table! {
    users_disliked_posts (disliked_id) {
        disliked_id -> Int4,
        post_id -> Int4,
        user_id -> Int8,
    }
}

table! {
    users_liked_posts (liked_id) {
        liked_id -> Int4,
        post_id -> Int4,
        user_id -> Int8,
    }
}

joinable!(download_stats -> posts (post_id));
joinable!(posts -> users (post_uploader));
joinable!(reports -> posts (post_id));
joinable!(reports -> users (user_id));
joinable!(users_disliked_posts -> posts (post_id));
joinable!(users_disliked_posts -> users (user_id));
joinable!(users_liked_posts -> posts (post_id));
joinable!(users_liked_posts -> users (user_id));

allow_tables_to_appear_in_same_query!(
    download_stats,
    post_dependencies,
    posts,
    reports,
    users,
    users_disliked_posts,
    users_liked_posts,
);
