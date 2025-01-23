CREATE TABLE reports (
	id serial primary key unique,
	post_id int not null references posts on delete cascade,
	user_id bigint not null references users on delete cascade,
	text text not null,
	time timestamp not null,
    admin_handled bigint references users on delete cascade
);
