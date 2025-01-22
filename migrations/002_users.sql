ALTER TABLE users ADD display_name text;
ALTER TABLE users ADD public_likes bool not null DEFAULT true;
ALTER TABLE users ADD show_explicit bool not null DEFAULT false;
