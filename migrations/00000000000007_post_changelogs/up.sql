-- Your SQL goes here
CREATE TABLE post_changelogs (
	changelog_id SERIAL PRIMARY KEY,
	post_id INT NOT NULL REFERENCES posts ON DELETE CASCADE,
	description TEXT NOT NULL,
	time TIMESTAMP NOT NULL DEFAULT NOW()
);
