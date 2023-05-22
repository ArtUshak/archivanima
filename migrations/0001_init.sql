CREATE TABLE users (
    username VARCHAR(64) PRIMARY KEY,
    password_hash TEXT NOT NULL,
    is_active BOOLEAN NOT NULL,
    is_uploader BOOLEAN NOT NULL,
    is_admin BOOLEAN NOT NULL,
    birth_date TIMESTAMP WITH TIME ZONE
);

CREATE TABLE invite_codes (
    invite_code VARCHAR(64) PRIMARY KEY
);

CREATE TABLE ban_reasons (
    id VARCHAR(64) PRIMARY KEY,
    description TEXT
);

CREATE TYPE upload_status AS ENUM ('INITIALIZED', 'ALLOCATED', 'WRITING', 'PUBLISHING', 'PUBLISHED', 'HIDING', 'HIDDEN', 'MISSING');

CREATE TABLE posts (
    id BIGSERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    creation_date TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    description TEXT NOT NULL,
    author_username VARCHAR(64) REFERENCES users (username) NOT NULL,
    is_hidden BOOLEAN NOT NULL,
    is_banned BOOLEAN NOT NULL,
    ban_reason_id VARCHAR(64) REFERENCES ban_reasons (id),
    ban_reason_text TEXT,
    min_age INTEGER CHECK(min_age >= 0 AND min_age <= 21),
    document_tsvector TSVECTOR NOT NULL
);

CREATE INDEX posts_index ON posts USING GIN (document_tsvector);

CREATE TABLE uploads (
    id BIGSERIAL PRIMARY KEY,
    extension VARCHAR(32),
    creation_date TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    size BIGINT NOT NULL,
    file_status upload_status NOT NULL,
    post_id BIGINT REFERENCES posts (id) NOT NULL
);

CREATE FUNCTION is_age_restricted(birth_date TIMESTAMP WITH TIME ZONE, current_date_arg TIMESTAMP WITH TIME ZONE, min_age INTEGER)
    RETURNS BOOLEAN
    LANGUAGE SQL
    IMMUTABLE
    RETURN (min_age IS NOT NULL) AND ((birth_date IS NULL) OR (DATE_PART('YEAR', AGE(current_date_arg, birth_date)) < min_age));
