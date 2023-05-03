CREATE TABLE users (
    username VARCHAR(64) PRIMARY KEY,
    password_hash TEXT NOT NULL,
    is_active BOOLEAN NOT NULL,
    is_uploader BOOLEAN NOT NULL,
    is_admin BOOLEAN NOT NULL
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
    ban_reason_text TEXT
);

CREATE TABLE uploads (
    id BIGSERIAL PRIMARY KEY,
    extension VARCHAR(32),
    creation_date TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    size BIGINT NOT NULL,
    file_status upload_status NOT NULL,
    post_id BIGINT REFERENCES posts (id) NOT NULL
);
