-- Your SQL goes here
CREATE TABLE post_dependencies(
	post_id INT REFERENCES posts ON DELETE CASCADE NOT NULL,
	dependency_id INT REFERENCES posts ON DELETE CASCADE NOT NULL CHECK (dependency_id != post_id),
	PRIMARY KEY (post_id, dependency_id)
);
