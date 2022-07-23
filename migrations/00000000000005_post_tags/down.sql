-- This file should undo anything in `up.sql`
ALTER TABLE posts DROP COLUMN post_game_tag;
ALTER TABLE posts DROP COLUMN post_type_tag;
