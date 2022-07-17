-- Your SQL goes here
CREATE TABLE download_stats(
	post_id INT REFERENCES posts ON DELETE CASCADE NOT NULL,
	timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	PRIMARY KEY (post_id, timestamp)
);
