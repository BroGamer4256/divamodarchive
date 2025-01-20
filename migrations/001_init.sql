CREATE TABLE users (
	id bigint primary key unique not null,
	name text not null,
	avatar text not null
);

CREATE TABLE posts (
	id serial primary key unique,
	name text not null,
	text text not null,
	images text[] not null default '{}',
	file text not null,
	time timestamp not null,
	type int not null,
	download_count bigint not null default 0
);

CREATE TABLE post_authors (
	post_id int not null references posts on delete cascade,
	user_id bigint not null references users on delete cascade
);

CREATE TABLE post_dependencies (
	post_id int not null references posts on delete cascade,
	dependency_id int not null references posts on delete cascade
);

CREATE TABLE post_comments (
	id serial primary key unique,
	post_id int not null references posts on delete cascade,
	user_id bigint not null references users on delete cascade,
	text text not null,
	parent int references post_comments on delete cascade,
	time timestamp not null
);

CREATE TABLE liked_posts (
	post_id int not null references posts on delete cascade,
	user_id bigint not null references users on delete cascade
);
