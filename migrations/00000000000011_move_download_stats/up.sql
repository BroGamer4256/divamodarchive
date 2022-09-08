ALTER TABLE posts ADD post_downloads BIGINT NOT NULL DEFAULT 0;

DO
$$
DECLARE
	post record;
BEGIN
	FOR post IN SELECT posts.post_id,COUNT(DISTINCT download_stats.timestamp) FROM posts LEFT JOIN download_stats ON posts.post_id=download_stats.post_id GROUP BY posts.post_id
	LOOP
		UPDATE posts SET post_downloads=post.count WHERE post_id=post.post_id;
	END LOOP;
END;
$$;

DROP TABLE download_stats;
