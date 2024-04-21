ALTER TABLE posts
    ADD COLUMN is_pinned BOOLEAN NOT NULL DEFAULT false;

CREATE INDEX posts_is_pinned ON posts (is_pinned);
