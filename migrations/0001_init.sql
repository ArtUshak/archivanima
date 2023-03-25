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

CREATE TABLE objects (
    resource_hash VARCHAR(1024),
    resource_extension VARCHAR(32),
    is_banned BOOLEAN NOT NULL,
    ban_reason_id VARCHAR(64) REFERENCES ban_reasons (id),
    ban_reason_text TEXT,
    PRIMARY KEY (resource_hash, resource_extension)
);

CREATE TABLE posts (
    id BIGSERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    author_username VARCHAR(64) REFERENCES users (username) NOT NULL,
    is_hidden BOOLEAN NOT NULL,
    is_banned BOOLEAN NOT NULL,
    ban_reason_id VARCHAR(64) REFERENCES ban_reasons (id),
    ban_reason_text TEXT,
    attachment_hash VARCHAR(1024),
    attachment_extension VARCHAR(32),
    FOREIGN KEY (attachment_hash, attachment_extension) REFERENCES objects (resource_hash, resource_extension)
);
