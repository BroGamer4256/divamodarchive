CREATE TABLE reservations (
	user_id bigint not null references users on delete cascade,
	reservation_type int not null default 0,
	range_start int not null,
	length int not null,
	time timestamp not null
);
