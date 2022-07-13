-- Your SQL goes here
CREATE TABLE users (
	user_id bigint primary key unique not null,
	user_name text not null,
	user_avatar text not null
);

CREATE TABLE posts (
	post_id serial primary key unique not null,
	post_name text not null,
	post_text text not null,
	post_text_short text not null,
	post_image text not null,
	post_uploader bigint references users on delete cascade not null,
	post_link text not null
);

CREATE TABLE users_liked_posts (
    liked_id serial primary key unique not null,
	post_id serial references posts on delete cascade not null,
	user_id bigint references users on delete cascade not null
);

CREATE TABLE users_disliked_posts (
    disliked_id serial primary key unique not null,
	post_id serial references posts on delete cascade not null,
	user_id bigint references users on delete cascade not null
);