INSERT INTO users (user_id, user_name, user_avatar) VALUES (1, 'First user', '');
INSERT INTO users (user_id, user_name, user_avatar) VALUES (2, 'Second user', '');
INSERT INTO users (user_id, user_name, user_avatar) VALUES (3, 'Third user', '');

INSERT INTO posts (post_name, post_text, post_text_short, post_image, post_uploader, post_link) VALUES ('First mod', '1 Like 2 Dislikes', '1:2', '', 1, '');
INSERT INTO posts (post_name, post_text, post_text_short, post_image, post_uploader, post_link) VALUES ('Second mod', '3 Like 0 Dislikes', '3:0', '', 3, '');
INSERT INTO posts (post_name, post_text, post_text_short, post_image, post_uploader, post_link) VALUES ('Third mod', '1 Like 1 Dislikes', '1:1', '', 2, '');

INSERT INTO users_liked_posts (post_id, user_id) VALUES (1, 1);
INSERT INTO users_disliked_posts (post_id, user_id) VALUES (1, 2);
INSERT INTO users_disliked_posts (post_id, user_id) VALUES (1, 3);

INSERT INTO users_liked_posts (post_id, user_id) VALUES (2, 1);
INSERT into users_liked_posts (post_id, user_id) VALUES (2, 2);
INSERT INTO users_liked_posts (post_id, user_id) VALUES (2, 3);

INSERT INTO users_liked_posts (post_id, user_id) VALUES (3, 2);
INSERT INTO users_disliked_posts (post_id, user_id) VALUES (3, 1);
