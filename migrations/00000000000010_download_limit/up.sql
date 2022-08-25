CREATE TABLE download_limit (
	limit_id SERIAL PRIMARY KEY,
    ip TEXT NOT NULL,
    downloaded BIGINT NOT NULL,
    date TIMESTAMP NOT NULL DEFAULT NOW()
);
