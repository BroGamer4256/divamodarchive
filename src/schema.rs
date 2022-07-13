table! {
    posts (post_id) {
        post_id -> Int4,
        post_name -> Text,
        post_text -> Text,
        post_text_short -> Text,
        post_image -> Text,
        post_uploader -> Int8,
        post_link -> Text,
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

joinable!(posts -> users (post_uploader));
joinable!(users_disliked_posts -> posts (post_id));
joinable!(users_disliked_posts -> users (user_id));
joinable!(users_liked_posts -> posts (post_id));
joinable!(users_liked_posts -> users (user_id));

allow_tables_to_appear_in_same_query!(
    posts,
    users,
    users_disliked_posts,
    users_liked_posts,
);
