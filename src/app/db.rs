use std::collections::HashSet;

use argon2::{password_hash::SaltString, Argon2, PasswordHasher, PasswordVerifier};
use rand::thread_rng;
use rocket::{
    async_trait,
    form::FromFormField,
    http::{uri::Origin, Status},
    request::{self, FromRequest},
    uri, Request, State,
};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};

use crate::{
    auth::Authentication,
    utils::{
        form_extra_validation::IdSet,
        pagination::{Page, PageParams},
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewUser<'a> {
    pub username: &'a str,
    pub password: &'a str,
    pub is_active: bool,
    pub is_admin: bool,
    pub is_uploader: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserFull {
    pub username: String,
    pub is_active: bool,
    pub is_admin: bool,
    pub is_uploader: bool,
    pub password_hash: String,
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
}

impl User {
    pub fn detail_url(&self) -> Origin {
        uri!(crate::app::views::user_detail_get(&self.username))
    }

    pub fn edit_url(&self) -> Origin {
        uri!(crate::app::views::user_edit_get(&self.username))
    }
}

impl From<UserFull> for User {
    fn from(value: UserFull) -> Self {
        Self {
            username: value.username,
            is_active: value.is_active,
            is_admin: value.is_admin,
            is_uploader: value.is_uploader,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsernameAndInviteCheckError {
    UserAlreadyExists,
    InvalidInviteCode,
}

pub async fn try_add_user_check_username<'a>(
    new_user: NewUser<'a>,
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
    .fetch_optional(&mut transaction)
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
    .execute(&mut transaction)
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
    .fetch_optional(&mut transaction)
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
    .fetch_optional(&mut transaction)
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
    .execute(&mut transaction)
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
    username, is_active, is_admin, is_uploader, password_hash
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
    }))
}

pub async fn try_get_user(
    username: &str,
    pool: &Pool<Postgres>,
) -> Result<Option<User>, crate::error::Error> {
    let result = sqlx::query!(
        r#"
SELECT
    username, is_active, is_admin, is_uploader
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
    .fetch_optional(&mut transaction)
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
    .execute(&mut transaction)
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
    .fetch_optional(&mut transaction)
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
    .execute(&mut transaction)
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
                    request::Outcome::Success(pool_state) => match list_ban_reasons(pool_state)
                        .await
                    {
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
                        Err(err) => request::Outcome::Failure((Status::InternalServerError, err)),
                    },
                    request::Outcome::Failure((status, ())) => {
                        request::Outcome::Failure((status, crate::error::Error::PoolNotFound))
                    }
                    request::Outcome::Forward(()) => request::Outcome::Forward(()),
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
    .fetch_optional(&mut transaction)
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
    .execute(&mut transaction)
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
pub struct NewPost {
    pub title: String,
    pub description: String,
    pub is_hidden: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Post {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub author_username: String,
    pub is_hidden: bool,
    pub ban: Option<(Option<BanReason>, Option<String>)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostVisibility {
    Visible(Post),
    Hidden,
    Banned(Option<BanReason>, Option<String>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostEdit {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub is_hidden: bool,
}

impl Post {
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

    let items = sqlx::query!(
        r#"
SELECT
    posts.id, title, posts.description AS post_description, author_username,
    is_hidden, is_banned, ban_reason_id, ban_reason_text, ban_reasons.description AS ban_reason_description
FROM
    posts
    LEFT JOIN ban_reasons
        ON posts.ban_reason_id = ban_reasons.id
WHERE
    posts.id >= $2
    AND posts.id < ($1 + $2)
ORDER BY
    id
        "#,
        limit,
        offset
    )
    .fetch_all(pool)
    .await?
    .iter()
    .map(|record| Post {
        id: record.id,
        title: record.title.clone(),
        description: record.post_description.clone(),
        author_username: record.author_username.clone(),
        is_hidden: record.is_hidden,
        ban: if record.is_banned {
            Some((
                record.ban_reason_id.clone().map(|ban_reason_id| BanReason {
                    id: ban_reason_id,
                    description: record.ban_reason_description.clone(),
                }),
                record.ban_reason_text.clone(),
            ))
        } else {
            None
        },
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

pub async fn try_get_post(
    id: i64,
    pool: &Pool<Postgres>,
) -> Result<Option<Post>, crate::error::Error> {
    let result = sqlx::query!(
        r#"
SELECT
    posts.id, title, posts.description AS post_description, author_username,
    is_hidden, is_banned, ban_reason_id, ban_reason_text, ban_reasons.description AS ban_reason_description
FROM
    posts
    LEFT JOIN ban_reasons
        ON posts.ban_reason_id = ban_reasons.id
WHERE
    posts.id = $1
            "#,
            id
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|record| Post {
        id: record.id,
        title: record.title,
        description: record.post_description,
        author_username: record.author_username,
        is_hidden: record.is_hidden,
        ban: if record.is_banned {
            Some((
                record.ban_reason_id.map(|ban_reason_id| BanReason {
                    id: ban_reason_id,
                    description: record.ban_reason_description,
                }),
                record.ban_reason_text,
            ))
        } else {
            None
        },
    }))
}

pub async fn add_post(
    post: NewPost,
    user: User,
    pool: &Pool<Postgres>,
) -> Result<Post, crate::error::Error> {
    let result = sqlx::query!(
        r#"
INSERT INTO
    posts (title, description, is_hidden, is_banned, author_username)
VALUES
    ($1, $2, $3, $4, $5)
RETURNING id
            "#,
        post.title,
        post.description,
        post.is_hidden,
        false,
        user.username,
    )
    .fetch_one(pool)
    .await?;

    Ok(Post {
        id: result.id,
        title: post.title,
        description: Some(post.description),
        author_username: user.username,
        is_hidden: post.is_hidden,
        ban: None,
    })
}

pub async fn try_edit_post_check_exists_and_permission(
    post: PostEdit,
    user: &User,
    pool: &Pool<Postgres>,
) -> Result<Option<()>, crate::error::Error> {
    let record = sqlx::query!(
        r#"
SELECT
    id, author_username
FROM
    posts
WHERE
    id = $1
        "#,
        post.id
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

    sqlx::query!(
        r#"
UPDATE
    posts
SET
    title = $2, description = $3, is_hidden = $4
WHERE
    id = $1
            "#,
        post.id,
        post.title,
        post.description,
        post.is_hidden,
    )
    .execute(pool)
    .await?;

    Ok(Some(()))
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
