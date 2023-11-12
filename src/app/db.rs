use std::{collections::HashSet, time::Duration};

use argon2::{password_hash::SaltString, Argon2, PasswordHasher, PasswordVerifier};
use log::debug;
use rand::thread_rng;
use rocket::{
    async_trait,
    form::FromFormField,
    http::{uri::Origin, Status},
    request::{self, FromRequest},
    time::OffsetDateTime,
    uri, Request, State,
};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::types::PgInterval, Pool, Postgres};

use crate::{
    app::storage::get_file_url,
    auth::Authentication,
    utils::{
        form_extra_validation::IdSet,
        iter_group::IntoGroupLinkedHashMap,
        pagination::{Page, PageParams},
    },
    UploadStorage,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewUser<'a> {
    pub username: &'a str,
    pub password: &'a str,
    pub is_active: bool,
    pub is_admin: bool,
    pub is_uploader: bool,
    pub birth_date: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserFull {
    pub username: String,
    pub is_active: bool,
    pub is_admin: bool,
    pub is_uploader: bool,
    pub password_hash: String,
    pub birth_date: Option<OffsetDateTime>,
}

impl UserFull {
    pub fn check_password(&self, password: &str) -> Result<bool, crate::error::Error> {
        let hash = argon2::PasswordHash::new(&self.password_hash)?;
        let argon2 = Argon2::default();
        match argon2.verify_password(password.as_bytes(), &hash) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub is_active: bool,
    pub is_admin: bool,
    pub is_uploader: bool,
    pub birth_date: Option<OffsetDateTime>,
}

impl User {
    pub fn detail_url(&self) -> Origin {
        uri!(crate::app::views::user_detail_get(&self.username))
    }

    pub fn edit_url(&self) -> Origin {
        uri!(crate::app::views::user_edit_get(&self.username))
    }

    pub fn is_uploader(&self) -> bool {
        self.is_uploader || self.is_admin
    }
}

impl From<UserFull> for User {
    fn from(value: UserFull) -> Self {
        Self {
            username: value.username,
            is_active: value.is_active,
            is_admin: value.is_admin,
            is_uploader: value.is_uploader,
            birth_date: value.birth_date,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, FromFormField)]
pub enum UserStatus {
    #[field(value = "banned")]
    Banned,
    #[field(value = "user")]
    User,
    #[field(value = "uploader")]
    Uploader,
    #[field(value = "admin")]
    Admin,
}

impl UserStatus {
    pub fn get_options() -> Vec<(String, String)> {
        vec![
            ("banned".to_string(), "забанен".to_string()),
            ("user".to_string(), "обычный".to_string()),
            ("uploader".to_string(), "загружающий".to_string()),
            ("admin".to_string(), "администратор".to_string()),
        ]
    }

    pub fn get_option(self) -> String {
        match self {
            UserStatus::Banned => "banned",
            UserStatus::User => "user",
            UserStatus::Uploader => "uploader",
            UserStatus::Admin => "admin",
        }
        .to_string()
    }
}

impl From<User> for UserStatus {
    fn from(value: User) -> Self {
        if !value.is_active {
            Self::Banned
        } else if value.is_admin {
            Self::Admin
        } else if value.is_uploader {
            Self::Uploader
        } else {
            Self::User
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsernameAndInviteCheckError {
    UserAlreadyExists,
    InvalidInviteCode,
}

pub async fn try_add_user_check_username(
    new_user: NewUser<'_>,
    pool: &Pool<Postgres>,
) -> Result<Option<()>, crate::error::Error> {
    let mut transaction = pool.begin().await?;

    let already_exists = sqlx::query!(
        r#"
SELECT
    username
FROM
    users
WHERE
    username = $1
        "#,
        new_user.username
    )
    .fetch_optional(&mut *transaction)
    .await?
    .is_some();

    if already_exists {
        transaction.commit().await?;

        return Ok(None);
    }

    let salt;
    {
        let mut rng = thread_rng();
        salt = SaltString::generate(&mut rng);
    };
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(new_user.password.as_bytes(), &salt)?
        .to_string();

    sqlx::query!(
        r#"
INSERT INTO
    users (username, password_hash, is_active, is_admin, is_uploader)
VALUES
    ($1, $2, $3, $4, $5)
            "#,
        new_user.username,
        password_hash,
        new_user.is_active,
        new_user.is_admin,
        new_user.is_uploader
    )
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(Some(()))
}

pub async fn try_add_user_check_username_and_invite<'a>(
    new_user: NewUser<'a>,
    invite_code: &'a str,
    pool: &Pool<Postgres>,
) -> Result<Result<(), UsernameAndInviteCheckError>, crate::error::Error> {
    let mut transaction = pool.begin().await?;

    let already_exists = sqlx::query!(
        r#"
SELECT
    username
FROM
    users
WHERE
    username = $1
        "#,
        new_user.username
    )
    .fetch_optional(&mut *transaction)
    .await?
    .is_some();

    if already_exists {
        transaction.commit().await?;

        return Ok(Err(UsernameAndInviteCheckError::UserAlreadyExists));
    }

    let is_valid_code = sqlx::query!(
        r#"
SELECT
    invite_code
FROM
    invite_codes
WHERE
    invite_code = $1
    "#,
        invite_code
    )
    .fetch_optional(&mut *transaction)
    .await?
    .is_some();

    if !is_valid_code {
        return Ok(Err(UsernameAndInviteCheckError::InvalidInviteCode));
    }

    let salt;
    {
        let mut rng = thread_rng();
        salt = SaltString::generate(&mut rng);
    };
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(new_user.password.as_bytes(), &salt)?
        .to_string();

    sqlx::query!(
        r#"
INSERT INTO
    users (username, password_hash, is_active, is_admin, is_uploader, birth_date)
VALUES
    ($1, $2, $3, $4, $5, $6)
            "#,
        new_user.username,
        password_hash,
        new_user.is_active,
        new_user.is_admin,
        new_user.is_uploader,
        new_user.birth_date
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!(
        r#"
DELETE FROM
    invite_codes
WHERE
    invite_code = $1
    "#,
        invite_code
    )
    .execute(pool)
    .await?;

    transaction.commit().await?;

    Ok(Ok(()))
}

pub async fn try_get_user_full(
    username: &str,
    pool: &Pool<Postgres>,
) -> Result<Option<UserFull>, crate::error::Error> {
    let result = sqlx::query!(
        r#"
SELECT
    username, is_active, is_admin, is_uploader, password_hash, birth_date
FROM
    users
WHERE
    username = $1
        "#,
        username
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|user_data| UserFull {
        username: user_data.username,
        is_active: user_data.is_active,
        is_admin: user_data.is_admin,
        is_uploader: user_data.is_uploader,
        password_hash: user_data.password_hash,
        birth_date: user_data.birth_date,
    }))
}

pub async fn try_get_user(
    username: &str,
    pool: &Pool<Postgres>,
) -> Result<Option<User>, crate::error::Error> {
    let result = sqlx::query!(
        r#"
SELECT
    username, is_active, is_admin, is_uploader, birth_date
FROM
    users
WHERE
    username = $1
        "#,
        username
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|user_data| User {
        username: user_data.username,
        is_active: user_data.is_active,
        is_admin: user_data.is_admin,
        is_uploader: user_data.is_uploader,
        birth_date: user_data.birth_date,
    }))
}

pub async fn try_edit_user_check_exists(
    username: &str,
    status: UserStatus,
    pool: &Pool<Postgres>,
) -> Result<Option<()>, crate::error::Error> {
    let already_exists = sqlx::query!(
        r#"
SELECT
    username
FROM
    users
WHERE
    username = $1
        "#,
        username
    )
    .fetch_optional(pool)
    .await?
    .is_some();

    if !already_exists {
        return Ok(None);
    }

    sqlx::query!(
        r#"
UPDATE
    users
SET
    is_active = $2, is_uploader = $3, is_admin = $4
WHERE
    username = $1
        "#,
        username,
        status != UserStatus::Banned,
        status == UserStatus::Uploader,
        status == UserStatus::Admin,
    )
    .execute(pool)
    .await?;

    Ok(Some(()))
}

pub async fn change_user_password(
    username: &str,
    new_password: &str,
    pool: &Pool<Postgres>,
) -> Result<(), crate::error::Error> {
    let salt;
    {
        let mut rng = thread_rng();
        salt = SaltString::generate(&mut rng);
    };
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(new_password.as_bytes(), &salt)?
        .to_string();

    sqlx::query!(
        r#"
UPDATE
    users
SET
    password_hash = $2
WHERE
    username = $1
        "#,
        username,
        password_hash
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn try_add_invite_check_exists(
    invite_code: &str,
    pool: &Pool<Postgres>,
) -> Result<Option<()>, crate::error::Error> {
    let mut transaction = pool.begin().await?;

    let already_exists = sqlx::query!(
        r#"
SELECT
    invite_code
FROM
    invite_codes
WHERE
    invite_code = $1
        "#,
        invite_code
    )
    .fetch_optional(&mut *transaction)
    .await?
    .is_some();

    if already_exists {
        transaction.commit().await?;

        return Ok(None);
    }

    sqlx::query!(
        r#"
INSERT INTO
    invite_codes (invite_code)
VALUES
    ($1)
            "#,
        invite_code
    )
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(Some(()))
}

pub async fn try_remove_invite_check_exists(
    invite_code: &str,
    pool: &Pool<Postgres>,
) -> Result<Option<()>, crate::error::Error> {
    let mut transaction = pool.begin().await?;

    let already_exists = sqlx::query!(
        r#"
SELECT
    invite_code
FROM
    invite_codes
WHERE
    invite_code = $1
        "#,
        invite_code
    )
    .fetch_optional(&mut *transaction)
    .await?
    .is_some();

    if !already_exists {
        transaction.commit().await?;

        return Ok(None);
    }

    sqlx::query!(
        r#"
DELETE FROM
    invite_codes
WHERE
    invite_code = $1
            "#,
        invite_code
    )
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(Some(()))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BanReason {
    pub id: String,
    pub description: Option<String>,
}

impl BanReason {
    pub fn edit_url(&self) -> Origin {
        uri!(crate::app::views::ban_reason_edit_get(&self.id))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BanReasonIdSet {
    pub ids_set: HashSet<String>,
    pub option_list: Vec<(String, String)>,
}

impl IdSet for BanReasonIdSet {
    fn get_option_list(&self) -> Vec<(String, String)> {
        self.option_list.clone()
    }

    fn is_valid_id(&self, id: &str) -> bool {
        self.ids_set.contains(id)
    }
}

#[async_trait]
impl<'r> FromRequest<'r> for BanReasonIdSet {
    type Error = crate::error::Error;

    async fn from_request(req: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let result: &request::Outcome<Self, Self::Error> = req
            .local_cache_async(async {
                let pool_state_result: request::Outcome<&State<Pool<Postgres>>, ()> =
                    req.guard().await;
                match pool_state_result {
                    request::Outcome::Success(pool_state) => {
                        match list_ban_reasons(pool_state).await {
                            Ok(ban_reasons) => request::Outcome::Success(BanReasonIdSet {
                                ids_set: HashSet::from_iter(
                                    ban_reasons.iter().map(|ban_reason| &ban_reason.id).cloned(),
                                ),
                                option_list: ban_reasons
                                    .iter()
                                    .map(|ban_reason| {
                                        (
                                            ban_reason.id.clone(),
                                            if let Some(description) = &ban_reason.description {
                                                format!("{}: {}", ban_reason.id, description)
                                            } else {
                                                ban_reason.id.clone()
                                            },
                                        )
                                    })
                                    .collect(),
                            }),
                            Err(err) => request::Outcome::Error((Status::InternalServerError, err)),
                        }
                    }
                    request::Outcome::Error((status, ())) => {
                        request::Outcome::Error((status, crate::error::Error::PoolNotFound))
                    }
                    request::Outcome::Forward(status) => request::Outcome::Forward(status),
                }
            })
            .await;

        result.clone()
    }
}

pub async fn list_ban_reasons(
    pool: &Pool<Postgres>,
) -> Result<Vec<BanReason>, crate::error::Error> {
    let result = sqlx::query!(
        r#"
SELECT
    id, description
FROM
    ban_reasons
ORDER BY
    id
        "#
    )
    .fetch_all(pool)
    .await?
    .iter()
    .map(|record| BanReason {
        id: record.id.clone(),
        description: record.description.clone(),
    })
    .collect();

    Ok(result)
}

pub async fn try_get_ban_reason(
    id: &str,
    pool: &Pool<Postgres>,
) -> Result<Option<BanReason>, crate::error::Error> {
    let result = sqlx::query!(
        r#"
SELECT
    description
FROM
    ban_reasons
WHERE
    id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?
    .map(|record| BanReason {
        id: id.to_owned(),
        description: record.description,
    });

    Ok(result)
}

pub async fn try_add_ban_reason_check_exists(
    ban_reason: BanReason,
    pool: &Pool<Postgres>,
) -> Result<Option<()>, crate::error::Error> {
    let mut transaction = pool.begin().await?;

    let already_exists = sqlx::query!(
        r#"
SELECT
    id
FROM
    ban_reasons
WHERE
    id = $1
        "#,
        ban_reason.id
    )
    .fetch_optional(&mut *transaction)
    .await?
    .is_some();

    if already_exists {
        transaction.commit().await?;

        return Ok(None);
    }

    sqlx::query!(
        r#"
INSERT INTO
    ban_reasons (id, description)
VALUES
    ($1, $2)
            "#,
        ban_reason.id,
        ban_reason.description
    )
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(Some(()))
}

pub async fn try_edit_ban_reason_check_exists(
    ban_reason: BanReason,
    pool: &Pool<Postgres>,
) -> Result<Option<()>, crate::error::Error> {
    let already_exists = sqlx::query!(
        r#"
SELECT
    id
FROM
    ban_reasons
WHERE
    id = $1
        "#,
        ban_reason.id
    )
    .fetch_optional(pool)
    .await?
    .is_some();

    if !already_exists {
        return Ok(None);
    }

    sqlx::query!(
        r#"
UPDATE
    ban_reasons
SET
    description = $2
WHERE
    id = $1
            "#,
        ban_reason.id,
        ban_reason.description
    )
    .execute(pool)
    .await?;

    Ok(Some(()))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewPost<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub is_hidden: bool,
    pub min_age: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Post {
    pub id: i64,
    pub creation_date: OffsetDateTime,
    pub title: String,
    pub description: String,
    pub author_username: String,
    pub is_hidden: bool,
    pub ban: Option<(Option<BanReason>, Option<String>)>,
    pub uploads: Vec<Upload>,
    pub min_age: Option<i32>,
    pub is_age_restricted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PostVisibility {
    Visible(Post),
    Hidden,
    Banned(Option<BanReason>, Option<String>),
    AgeRestricted(i32),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostEdit<'r> {
    pub id: i64,
    pub title: Option<&'r str>,
    pub description: Option<&'r str>,
    pub is_hidden: Option<bool>,
    pub min_age: Option<i32>,
}

impl Post {
    pub fn detail_url(&self) -> Origin {
        uri!(crate::app::views::post_detail_get(self.id))
    }

    pub fn author_detail_url(&self) -> Origin {
        uri!(crate::app::views::user_detail_get(&self.author_username))
    }

    pub fn check_visible(self, user: &Authentication) -> PostVisibility {
        if user.is_admin() {
            PostVisibility::Visible(self)
        } else {
            match &self.ban {
                Some((ban_reason, ban_reason_text)) => {
                    PostVisibility::Banned(ban_reason.clone(), ban_reason_text.clone())
                }
                None => {
                    if self.is_hidden {
                        if Some(&self.author_username) == user.username().as_ref() {
                            PostVisibility::Visible(self)
                        } else {
                            PostVisibility::Hidden
                        }
                    } else if self.is_age_restricted {
                        if Some(&self.author_username) == user.username().as_ref() {
                            PostVisibility::Visible(self)
                        } else {
                            PostVisibility::AgeRestricted(self.min_age.unwrap_or(0))
                        }
                    } else {
                        PostVisibility::Visible(self)
                    }
                }
            }
        }
    }

    pub fn can_edit(&self, user: &Authentication) -> bool {
        match user {
            Authentication::Authenticated(user_real) => user_real.username == self.author_username,
            Authentication::Banned(_) => false,
            Authentication::Anonymous => false,
        }
    }

    pub fn can_edit_by_user(&self, user: &User) -> bool {
        user.username == self.author_username
    }

    pub fn can_ban(&self, user: &Authentication) -> bool {
        user.is_admin()
    }

    pub fn can_unban(&self, user: &Authentication) -> bool {
        user.is_admin() && self.ban.is_some()
    }
}

pub async fn list_posts_with_pagination(
    pool: &Pool<Postgres>,
    page_params: PageParams,
    user: &Authentication,
) -> Result<Page<Post>, crate::error::Error> {
    let count_query_result = sqlx::query!(
        r#"
SELECT
    COUNT(id), MAX(id)
FROM
    posts
        "#
    )
    .fetch_one(pool)
    .await?;
    let max_id = count_query_result.max.unwrap_or(0) as u64;
    let total_item_count = count_query_result.count.unwrap_or(0) as u64;

    let page_count = max_id.div_ceil(page_params.page_size);

    let (limit, offset, page_id) = page_params.get_limit_offset_and_page_id(page_count)?;

    let group_by = sqlx::query!(
        r#"
SELECT
    posts.id, posts.creation_date, title, posts.description AS post_description, author_username,
    is_hidden, is_banned, ban_reason_id, ban_reason_text, ban_reasons.description AS ban_reason_description,
    uploads.id AS "upload_id?", uploads.extension AS "upload_extension?", uploads.creation_date AS "upload_creation_date?",
    uploads.size AS "size?", uploads.file_status AS "file_status?: UploadStatus",
    min_age, is_age_restricted($3, CURRENT_TIMESTAMP, min_age) AS is_age_restricted
FROM
    posts
    LEFT JOIN ban_reasons
        ON posts.ban_reason_id = ban_reasons.id
    LEFT JOIN uploads
        ON posts.id = uploads.post_id
        AND file_status = 'PUBLISHED'
WHERE
    posts.id >= $2
    AND posts.id < ($1 + $2)
ORDER BY
    posts.id, uploads.id
        "#,
        limit,
        offset,
        user.birth_date()
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|record| (
        (
            record.id, record.creation_date, record.title, record.post_description, record.author_username, record.is_hidden,
            record.is_banned, record.ban_reason_id, record.ban_reason_description, record.ban_reason_text, record.min_age,
            record.is_age_restricted
        ),
        match record.upload_id {
            Some(upload_id) => Some((upload_id, record.upload_extension, record.upload_creation_date, record.size, record.file_status)),
            None => None
        }
    ))
    .into_group_linked_map();

    let items: Vec<Post> = group_by
        .into_iter()
        .map(
            |(
                (
                    post_id,
                    creation_date,
                    title,
                    post_description,
                    author_username,
                    is_hidden,
                    is_banned,
                    ban_reason_id,
                    ban_reason_description,
                    ban_reason_text,
                    min_age,
                    is_age_restricted,
                ),
                upload_records,
            )| Post {
                id: post_id,
                creation_date,
                title,
                description: post_description,
                author_username,
                is_hidden,
                ban: if is_banned {
                    Some((
                        ban_reason_id.map(|ban_reason_id| BanReason {
                            id: ban_reason_id,
                            description: ban_reason_description,
                        }),
                        ban_reason_text,
                    ))
                } else {
                    None
                },
                uploads: upload_records
                    .into_iter()
                    .flatten()
                    .map(
                        |(upload_id, extension, upload_creation_date, size, file_status)| Upload {
                            id: upload_id,
                            extension,
                            size: size.unwrap(),
                            creation_date: upload_creation_date.unwrap(),
                            file_status: file_status.unwrap(),
                        },
                    )
                    .collect(),
                min_age,
                is_age_restricted: is_age_restricted.unwrap(),
            },
        )
        .collect();

    Ok(Page {
        items,
        page_id,
        page_size: page_params.page_size,
        total_item_count,
        page_count,
    })
}

pub async fn search_posts_with_pagination(
    pool: &Pool<Postgres>,
    query: Option<&str>,
    page_params: PageParams,
    user: &Authentication,
) -> Result<Page<Post>, crate::error::Error> {
    let count_query_result = sqlx::query!(
        r#"
SELECT
    COUNT(id)
FROM
    posts, to_tsquery($1) query
WHERE
    query @@ document_tsvector
        "#,
        query
    )
    .fetch_one(pool)
    .await?;
    let total_item_count = count_query_result.count.unwrap_or(0) as u64;

    let page_count = total_item_count.div_ceil(page_params.page_size);

    let (limit, offset, page_id) = page_params.get_limit_offset_and_page_id(page_count)?;

    let group_by = sqlx::query!(
        r#"
SELECT
    posts.id, posts.creation_date, title,
    posts.description AS post_description, author_username,
    is_hidden, is_banned, ban_reason_id, ban_reason_text, ban_reasons.description AS ban_reason_description,
    uploads.id AS "upload_id?", uploads.extension AS "upload_extension?", uploads.creation_date AS "upload_creation_date?",
    uploads.size AS "size?", uploads.file_status AS "file_status?: UploadStatus",
    min_age, is_age_restricted($3, CURRENT_TIMESTAMP, min_age) AS is_age_restricted
FROM
    (
        SELECT
            id, creation_date, title, description, author_username,
            is_hidden, is_banned, ban_reason_id, ban_reason_text, min_age,
            ts_rank(document_tsvector, query) AS rank
        FROM
            posts, to_tsquery($4) query
        WHERE
            query @@ document_tsvector
        ORDER BY
            rank DESC, id ASC
        LIMIT
            $1
        OFFSET
            $2
    ) posts
    LEFT JOIN ban_reasons
        ON posts.ban_reason_id = ban_reasons.id
    LEFT JOIN uploads
        ON posts.id = uploads.post_id
        AND file_status = 'PUBLISHED'
ORDER BY
    rank DESC, posts.id ASC, uploads.id ASC
        "#,
        limit,
        offset,
        user.birth_date(),
        query
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|record|
        (
            (
                record.id, record.creation_date, record.title, record.post_description, record.author_username, record.is_hidden,
                record.is_banned, record.ban_reason_id, record.ban_reason_description, record.ban_reason_text, record.min_age,
                record.is_age_restricted
            ),
            match record.upload_id {
                Some(upload_id) => Some((upload_id, record.upload_extension, record.upload_creation_date, record.size, record.file_status)),
                None => None
            }
        )
    )
    .into_group_linked_map();

    let items: Vec<Post> = group_by
        .into_iter()
        .map(
            |(
                (
                    post_id,
                    creation_date,
                    title,
                    post_description,
                    author_username,
                    is_hidden,
                    is_banned,
                    ban_reason_id,
                    ban_reason_description,
                    ban_reason_text,
                    min_age,
                    is_age_restricted,
                ),
                upload_records,
            )| Post {
                id: post_id,
                creation_date,
                title,
                description: post_description,
                author_username,
                is_hidden,
                ban: if is_banned {
                    Some((
                        ban_reason_id.map(|ban_reason_id| BanReason {
                            id: ban_reason_id,
                            description: ban_reason_description,
                        }),
                        ban_reason_text,
                    ))
                } else {
                    None
                },
                uploads: upload_records
                    .into_iter()
                    .flatten()
                    .map(
                        |(upload_id, extension, upload_creation_date, size, file_status)| Upload {
                            id: upload_id,
                            extension,
                            size: size.unwrap(),
                            creation_date: upload_creation_date.unwrap(),
                            file_status: file_status.unwrap(),
                        },
                    )
                    .collect(),
                min_age,
                is_age_restricted: is_age_restricted.unwrap(),
            },
        )
        .collect();

    Ok(Page {
        items,
        page_id,
        page_size: page_params.page_size,
        total_item_count,
        page_count,
    })
}

pub async fn try_get_post(
    id: i64,
    pool: &Pool<Postgres>,
    user: &Authentication,
) -> Result<Option<Post>, crate::error::Error> {
    let result = sqlx::query!(
        r#"
SELECT
    posts.id, posts.creation_date, title, posts.description AS post_description, author_username,
    is_hidden, is_banned, ban_reason_id, ban_reason_text, ban_reasons.description AS ban_reason_description,
    uploads.id AS "upload_id?", uploads.extension AS "upload_extension?", uploads.creation_date AS "upload_creation_date?",
    uploads.size AS "size?", uploads.file_status AS "file_status?: UploadStatus",
    min_age, is_age_restricted($2, CURRENT_TIMESTAMP, min_age) AS is_age_restricted
FROM
    posts
    LEFT JOIN ban_reasons
        ON posts.ban_reason_id = ban_reasons.id
    LEFT JOIN uploads
    ON posts.id = uploads.post_id
    AND file_status = 'PUBLISHED'
WHERE
    posts.id = $1
            "#,
            id,
            user.birth_date()
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|record| (
        (
            record.id, record.creation_date, record.title, record.post_description, record.author_username, record.is_hidden,
            record.is_banned, record.ban_reason_id, record.ban_reason_description, record.ban_reason_text,
            record.min_age, record.is_age_restricted
        ),
        match record.upload_id {
            Some(upload_id) => Some((upload_id, record.upload_extension, record.upload_creation_date, record.size, record.file_status)),
            None => None
        }
    ))
    .into_group_linked_map();

    let mut result_iter = result.into_iter();

    Ok(result_iter.next().map(
        |(
            (
                post_id,
                creation_date,
                title,
                post_description,
                author_username,
                is_hidden,
                is_banned,
                ban_reason_id,
                ban_reason_description,
                ban_reason_text,
                min_age,
                is_age_restricted,
            ),
            upload_records,
        )| Post {
            id: post_id,
            creation_date,
            title,
            description: post_description,
            author_username,
            is_hidden,
            ban: if is_banned {
                Some((
                    ban_reason_id.map(|ban_reason_id| BanReason {
                        id: ban_reason_id,
                        description: ban_reason_description,
                    }),
                    ban_reason_text,
                ))
            } else {
                None
            },
            uploads: upload_records
                .into_iter()
                .flatten()
                .map(
                    |(upload_id, extension, upload_creation_date, size, file_status)| Upload {
                        id: upload_id,
                        extension,
                        size: size.unwrap(),
                        creation_date: upload_creation_date.unwrap(),
                        file_status: file_status.unwrap(),
                    },
                )
                .collect(),
            min_age,
            is_age_restricted: is_age_restricted.unwrap(),
        },
    ))
}

pub async fn add_post(
    post: NewPost<'_>,
    user: User,
    pool: &Pool<Postgres>,
) -> Result<Post, crate::error::Error> {
    let result = sqlx::query!(
        r#"
INSERT INTO
    posts (title, description, is_hidden, is_banned, author_username, min_age, document_tsvector)
VALUES
    ($1, $2, $3, $4, $5, $6, TO_TSVECTOR($1 || ' ' || COALESCE($2, '')))
RETURNING id, creation_date
            "#,
        post.title,
        post.description,
        post.is_hidden,
        false,
        user.username,
        post.min_age,
    )
    .fetch_one(pool)
    .await?;

    Ok(Post {
        id: result.id,
        creation_date: result.creation_date,
        title: post.title.to_string(),
        description: post.description.to_string(),
        author_username: user.username,
        is_hidden: post.is_hidden,
        ban: None,
        uploads: vec![],
        min_age: post.min_age,
        is_age_restricted: false,
    })
}

pub async fn try_edit_post_check_exists_and_permission<'r>(
    post: PostEdit<'r>,
    user: &User,
    pool: &Pool<Postgres>,
) -> Result<(), crate::error::Error> {
    let record = sqlx::query!(
        r#"
SELECT
    id, author_username, title, description, is_hidden
FROM
    posts
WHERE
    id = $1
        "#,
        post.id
    )
    .fetch_optional(pool)
    .await?;

    let record = record.ok_or(crate::error::Error::DoesNotExist)?;

    if record.author_username != user.username {
        return Err(crate::error::Error::AccessDenied);
    }

    sqlx::query!(
        r#"
UPDATE
    posts
SET
    title = $2, description = $3, is_hidden = $4, min_age = $5,
    document_tsvector = TO_TSVECTOR($2 || ' ' || COALESCE($3, ''))
WHERE
    id = $1
            "#,
        post.id,
        post.title.unwrap_or(&record.title),
        post.description.unwrap_or(&record.description),
        post.is_hidden.unwrap_or(record.is_hidden),
        post.min_age
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn try_ban_post_check_exists(
    post_id: i64,
    ban_reason_id: Option<String>,
    ban_reason_text: Option<String>,
    pool: &Pool<Postgres>,
) -> Result<Option<()>, crate::error::Error> {
    let post_already_exists = sqlx::query!(
        r#"
SELECT
    id
FROM
    posts
WHERE
    id = $1
        "#,
        post_id
    )
    .fetch_optional(pool)
    .await?
    .is_some();

    if !post_already_exists {
        return Ok(None);
    }

    sqlx::query!(
        r#"
UPDATE
    posts
SET
    is_banned = TRUE, ban_reason_id = $2, ban_reason_text = $3
WHERE
    id = $1
        "#,
        post_id,
        ban_reason_id,
        ban_reason_text
    )
    .fetch_optional(pool)
    .await?;

    Ok(Some(()))
}

pub async fn try_unban_post_check_exists(
    post_id: i64,
    pool: &Pool<Postgres>,
) -> Result<Option<()>, crate::error::Error> {
    let post_already_exists = sqlx::query!(
        r#"
SELECT
    id
FROM
    posts
WHERE
    id = $1
        "#,
        post_id
    )
    .fetch_optional(pool)
    .await?
    .is_some();

    if !post_already_exists {
        return Ok(None);
    }

    sqlx::query!(
        r#"
UPDATE
    posts
SET
    is_banned = FALSE, ban_reason_id = NULL, ban_reason_text = NULL
WHERE
    id = $1
        "#,
        post_id,
    )
    .fetch_optional(pool)
    .await?;

    Ok(Some(()))
}

#[derive(Clone, Debug, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "upload_status")]
#[sqlx(rename_all = "UPPERCASE")]
pub enum UploadStatus {
    Initialized,
    Allocated,
    Writing,
    Publishing,
    Published,
    Hiding,
    Hidden,
    Missing,
}

impl UploadStatus {
    pub fn can_transition_to(&self, new_status: &UploadStatus) -> bool {
        match new_status {
            UploadStatus::Initialized => false,
            UploadStatus::Allocated => {
                self == &UploadStatus::Initialized || self == &UploadStatus::Writing
            }
            UploadStatus::Writing => self == &UploadStatus::Allocated,
            UploadStatus::Publishing => {
                self == &UploadStatus::Allocated || self == &UploadStatus::Hidden
            }
            UploadStatus::Published => self == &UploadStatus::Publishing,
            UploadStatus::Hiding => self == &UploadStatus::Published,
            UploadStatus::Hidden => self == &UploadStatus::Hiding,
            UploadStatus::Missing => true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewUpload<'a> {
    pub extension: Option<&'a str>,
    pub size: i64,
    pub post_id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Upload {
    pub id: i64,
    pub extension: Option<String>,
    pub size: i64,
    pub creation_date: OffsetDateTime,
    pub file_status: UploadStatus,
}

impl Upload {
    pub fn file_url(&self, storage: &UploadStorage) -> String {
        get_file_url(self.id, self.extension.as_deref(), storage)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadFull {
    pub id: i64,
    pub extension: Option<String>,
    pub size: i64,
    pub creation_date: OffsetDateTime,
    pub file_status: UploadStatus,
    pub post_id: i64,
    pub post_author_username: String,
}

pub async fn get_upload(id: i64, pool: &Pool<Postgres>) -> Result<UploadFull, crate::error::Error> {
    let result = sqlx::query!(
        r#"
SELECT
    file_status AS "file_status: UploadStatus", extension, uploads.creation_date, size, post_id, posts.author_username
FROM
    uploads
    JOIN posts
        ON posts.id = uploads.post_id
WHERE
    uploads.id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    match result {
        None => Err(crate::error::Error::DoesNotExist),
        Some(record) => Ok(UploadFull {
            id,
            extension: record.extension,
            size: record.size,
            creation_date: record.creation_date,
            file_status: record.file_status,
            post_id: record.post_id,
            post_author_username: record.author_username,
        }),
    }
}

pub async fn add_upload(
    upload: NewUpload<'_>,
    user: User,
    pool: &Pool<Postgres>,
) -> Result<Upload, crate::error::Error> {
    let record = sqlx::query!(
        r#"
SELECT
    id, author_username
FROM
    posts
WHERE
    id = $1
        "#,
        upload.post_id
    )
    .fetch_optional(pool)
    .await?;

    let author_username = match record {
        Some(record_real) => Ok(record_real.author_username),
        None => Err(crate::error::Error::DoesNotExist),
    }?;

    if author_username != user.username {
        return Err(crate::error::Error::AccessDenied);
    }

    let result = sqlx::query!(
        r#"
INSERT INTO
    uploads (extension, size, file_status, post_id)
VALUES
    ($1, $2, $3, $4)
RETURNING id, creation_date
            "#,
        upload.extension,
        upload.size,
        UploadStatus::Initialized as _,
        upload.post_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(Upload {
        id: result.id,
        extension: upload.extension.map(|x| x.to_string()),
        size: upload.size,
        creation_date: result.creation_date,
        file_status: UploadStatus::Initialized,
    })
}

pub async fn try_set_upload_status(
    id: i64,
    new_status: UploadStatus,
    pool: &Pool<Postgres>,
) -> Result<Option<()>, crate::error::Error> {
    let mut transaction = pool.begin().await?;

    let file_status = sqlx::query!(
        r#"
SELECT
    file_status AS "file_status: UploadStatus"
FROM
    uploads
WHERE
    id = $1
        "#,
        id
    )
    .fetch_one(&mut *transaction)
    .await?
    .file_status;

    debug!(
        "Trying to transition upload {} from {:?} to {:?}",
        id, file_status, new_status
    );
    if !file_status.can_transition_to(&new_status) {
        transaction.commit().await?;

        return Ok(None);
    }

    sqlx::query!(
        r#"
UPDATE
    uploads
SET
    file_status = $2
WHERE
    id = $1
            "#,
        id,
        new_status as _
    )
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(Some(()))
}

pub async fn try_set_upload_status_check_exists(
    id: i64,
    new_status: UploadStatus,
    pool: &Pool<Postgres>,
) -> Result<Option<UploadFull>, crate::error::Error> {
    let mut transaction = pool.begin().await?;

    let result = sqlx::query!(
        r#"
SELECT
    file_status AS "file_status: UploadStatus", extension, uploads.creation_date, size, post_id, posts.author_username
FROM
    uploads
    JOIN posts
        ON posts.id = uploads.post_id
WHERE
    uploads.id = $1
        "#,
        id
    )
    .fetch_optional(&mut *transaction)
    .await?;

    match result {
        None => Err(crate::error::Error::DoesNotExist),
        Some(record) => {
            if !record.file_status.can_transition_to(&new_status) {
                Ok(None)
            } else {
                sqlx::query!(
                    r#"
UPDATE
    uploads
SET
    file_status = $2
WHERE
    id = $1
            "#,
                    id,
                    new_status as _
                )
                .execute(&mut *transaction)
                .await?;

                transaction.commit().await?;

                Ok(Some(UploadFull {
                    id,
                    extension: record.extension,
                    size: record.size,
                    creation_date: record.creation_date,
                    file_status: record.file_status,
                    post_id: record.post_id,
                    post_author_username: record.author_username,
                }))
            }
        }
    }
}

pub async fn list_old_in_progress_uploads_and_set_hiding(
    pool: &Pool<Postgres>,
    page_params: PageParams,
    max_age: Duration,
) -> Result<Page<Upload>, crate::error::Error> {
    let max_age: PgInterval = max_age.try_into()?;

    let count_query_result = sqlx::query!(
        r#"
SELECT
    COUNT(id)
FROM
    uploads
WHERE
    file_status NOT IN ('PUBLISHED', 'HIDDEN', 'MISSING')
    AND (
        AGE(CURRENT_TIMESTAMP, creation_date) > $1
        OR file_status = 'HIDING'
    )
        "#,
        max_age
    )
    .fetch_one(pool)
    .await?;

    let total_item_count = count_query_result.count.unwrap_or(0) as u64;

    let page_count = total_item_count.div_ceil(page_params.page_size);

    let (limit, offset, page_id) = page_params.get_limit_offset_and_page_id(page_count)?;

    let items = sqlx::query!(
        r#"
UPDATE
    uploads
SET
    file_status = 'HIDING'
WHERE
    id IN (
        SELECT
            id 
        FROM
            uploads
        WHERE
            file_status NOT IN ('PUBLISHED', 'HIDDEN', 'MISSING')
            AND (
                AGE(CURRENT_TIMESTAMP, creation_date) > $3
                OR file_status = 'HIDING'
            )
        ORDER BY
            id
        LIMIT
            $1
        OFFSET
            $2
    )
RETURNING
    id, extension, creation_date, size, file_status AS "file_status: UploadStatus"
        "#,
        limit,
        offset,
        max_age
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|record| Upload {
        id: record.id,
        extension: record.extension,
        size: record.size,
        creation_date: record.creation_date,
        file_status: record.file_status,
    })
    .collect();

    Ok(Page {
        items,
        page_id,
        page_size: page_params.page_size,
        total_item_count,
        page_count,
    })
}

pub async fn set_uploads_hidden(
    pool: &Pool<Postgres>,
    ids: Vec<i64>,
) -> Result<(), crate::error::Error> {
    sqlx::query!(
        r#"
UPDATE
    uploads
SET
    file_status = 'HIDDEN'
WHERE
    id = ANY($1::BIGINT[])
        "#,
        ids.as_slice()
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_users_with_pagination(
    pool: &Pool<Postgres>,
    page_params: PageParams,
) -> Result<Page<User>, crate::error::Error> {
    let count_query_result = sqlx::query!(
        r#"
SELECT
    COUNT(id), MAX(id)
FROM
    posts
        "#
    )
    .fetch_one(pool)
    .await?;
    let max_id = count_query_result.max.unwrap_or(0) as u64;
    let total_item_count = count_query_result.count.unwrap_or(0) as u64;

    let page_count = max_id.div_ceil(page_params.page_size);

    let (limit, offset, page_id) = page_params.get_limit_offset_and_page_id(page_count)?;

    let items = sqlx::query!(
        r#"
SELECT
    username, is_active, is_admin, is_uploader, password_hash, birth_date
FROM
    users
ORDER BY
    username
LIMIT $2
OFFSET $1
    "#,
        offset,
        limit
    )
    .fetch_all(pool)
    .await?
    .iter()
    .map(|user_data| User {
        username: user_data.username.clone(),
        is_active: user_data.is_active,
        is_admin: user_data.is_admin,
        is_uploader: user_data.is_uploader,
        birth_date: user_data.birth_date,
    })
    .collect();

    Ok(Page {
        items,
        page_id,
        page_size: page_params.page_size,
        total_item_count,
        page_count,
    })
}
