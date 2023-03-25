use argon2::{password_hash::SaltString, Argon2, PasswordHasher, PasswordVerifier};
use rand::thread_rng;
use rocket::{http::uri::Origin, uri};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};

use crate::{
    auth::Authentication,
    error,
    utils::pagination::{Page, PageParams},
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
    pub fn check_password(&self, password: &str) -> Result<bool, error::Error> {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsernameAndInviteCheckError {
    UserAlreadyExists,
    InvalidInviteCode,
}

pub async fn try_add_user_check_username<'a>(
    new_user: NewUser<'a>,
    pool: &Pool<Postgres>,
) -> Result<Option<()>, error::Error> {
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
) -> Result<Result<(), UsernameAndInviteCheckError>, error::Error> {
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
) -> Result<Option<UserFull>, error::Error> {
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
) -> Result<Option<User>, error::Error> {
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

    Ok(result.map(|user_data| User {
        username: user_data.username,
        is_active: user_data.is_active,
        is_admin: user_data.is_admin,
        is_uploader: user_data.is_uploader,
    }))
}

pub async fn try_add_invite_check_exists(
    invite_code: &str,
    pool: &Pool<Postgres>,
) -> Result<Option<()>, error::Error> {
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
) -> Result<Option<()>, error::Error> {
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
    pub fn get_edit_url(&self) -> Origin {
        uri!(crate::app::views::ban_reason_edit_get(&self.id))
    }
}

pub async fn list_ban_reasons(pool: &Pool<Postgres>) -> Result<Vec<BanReason>, error::Error> {
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

pub async fn get_ban_reason(
    id: &str,
    pool: &Pool<Postgres>,
) -> Result<Option<BanReason>, error::Error> {
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
) -> Result<Option<()>, error::Error> {
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
) -> Result<Option<()>, error::Error> {
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

    if !already_exists {
        transaction.commit().await?;

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
    .execute(&mut transaction)
    .await?;

    transaction.commit().await?;

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

impl Post {
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
}

pub async fn list_posts_with_pagination(
    pool: &Pool<Postgres>,
    page_params: PageParams,
) -> Result<Page<Post>, error::Error> {
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

pub async fn get_post(id: i64, pool: &Pool<Postgres>) -> Result<Option<Post>, error::Error> {
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
) -> Result<Post, error::Error> {
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

    println!("!!");

    Ok(Post {
        id: result.id,
        title: post.title,
        description: Some(post.description),
        author_username: user.username,
        is_hidden: post.is_hidden,
        ban: None,
    })
}
