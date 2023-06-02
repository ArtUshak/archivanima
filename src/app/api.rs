use std::{borrow::Cow, cmp::max, collections::HashMap};

use lazy_static::lazy_static;
use maplit::hashmap;
use regex::Regex;
use rocket::{data::ToByteUnit, post, put, serde::json::Json, Data, Either, State};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Pool, Postgres};
use validator::{Validate, ValidationError, ValidationErrors};

use crate::{
    app::{
        db::{
            add_post, add_upload, get_upload, try_edit_post_check_exists_and_permission,
            try_set_upload_status, try_set_upload_status_check_exists, NewPost, NewUpload,
            PostEdit, UploadStatus, User,
        },
        storage::{allocate_private_file, publish_file, unpublish_file, write_private_file},
    },
    auth::Uploader,
    utils::{content_range::ContentRange, csrf::HeaderCSRF},
    UploadConfig,
};

lazy_static! {
    static ref EXTENSION_REGEX: Regex = Regex::new(r"^[a-zA-Z0-9_]+$").unwrap();
}

#[derive(Clone, Debug, Deserialize, Serialize, Validate)]
pub struct PostAddRequest<'r> {
    #[validate(length(
        min = 1,
        code = "title_is_blank",
        message = "название не должно быть пустым"
    ))]
    #[validate(length(
        max = 500,
        code = "title_too_long",
        message = "название должно быть не длиннее 500 символов"
    ))]
    title: &'r str,

    description: &'r str,

    is_hidden: bool,

    #[validate(range(
        min = 0,
        max = 21,
        message = "минимальный возраст должен быть в диапазоне от 0 до 21 года включительно"
    ))]
    min_age: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PostAddResponseOk {
    id: i64,
    url: String,
}

#[post("/api/posts/add", data = "<request>")]
pub async fn post_add_post<'r, 'a, 'b>(
    request: Json<PostAddRequest<'r>>,
    pool: &'a State<Pool<Postgres>>,
    user: User,
    _uploader: Uploader,
    _header_csrf: HeaderCSRF,
) -> Result<Json<PostAddResponseOk>, crate::error::Error> {
    request.validate()?;

    let post = add_post(
        NewPost {
            title: request.title,
            description: request.description,
            is_hidden: request.is_hidden,
            min_age: request.min_age,
        },
        user,
        pool,
    )
    .await?;

    Ok(Json(PostAddResponseOk {
        id: post.id,
        url: post.detail_url().to_string(),
    }))
}

#[derive(Clone, Debug, Deserialize, Serialize, Validate)]
pub struct PostEditRequest {
    #[validate(length(
        min = 1,
        code = "title_is_blank",
        message = "название не должно быть пустым"
    ))]
    #[validate(length(
        max = 500,
        code = "title_too_long",
        message = "название должно быть не длиннее 500 символов"
    ))]
    title: Option<String>,

    description: Option<String>,

    is_hidden: Option<bool>,

    #[validate(range(
        min = 0,
        max = 21,
        message = "минимальный возраст должен быть в диапазоне от 0 до 21 года включительно"
    ))]
    min_age: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PostEditResponseOk {}

#[post("/api/posts/by-id/<id>/edit", data = "<request>")]
pub async fn post_edit_post<'b>(
    id: i64,
    request: Json<PostEditRequest>,
    pool: &State<Pool<Postgres>>,
    user: User,
    _uploader: Uploader,
    _header_csrf: HeaderCSRF,
) -> Result<Json<PostEditResponseOk>, crate::error::Error> {
    request.validate()?;

    try_edit_post_check_exists_and_permission(
        PostEdit {
            id,
            title: request.title.as_deref(),
            description: request.description.as_deref(),
            is_hidden: request.is_hidden,
            min_age: request.min_age,
        },
        &user,
        pool,
    )
    .await?;

    Ok(Json(PostEditResponseOk {}))
}

#[derive(Clone, Debug, Deserialize, Serialize, Validate)]
pub struct UploadAddRequest<'r> {
    size: u64,

    #[validate(length(max = 32, code = "extension_too_long"))]
    #[validate(regex(path = "EXTENSION_REGEX", code = "extension_invalid_chars"))]
    extension: Option<&'r str>,

    post_id: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UploadAddResponseOk {
    id: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UploadByChunkResponseOk {}

#[post("/api/uploads/add", data = "<request>")]
pub async fn upload_add_post<'r, 'a, 'b>(
    request: Json<UploadAddRequest<'r>>,
    pool: &'a State<Pool<Postgres>>,
    user: User,
    upload_config: &'b State<UploadConfig>,
    _uploader: Uploader,
    _header_csrf: HeaderCSRF,
) -> Result<Json<UploadAddResponseOk>, crate::error::Error> {
    let mut validation_errors = request
        .validate()
        .err()
        .unwrap_or_else(ValidationErrors::new);

    if request.size == 0 {
        validation_errors.add(
            "size",
            ValidationError {
                code: Cow::from("size_is_zero"),
                message: None,
                params: HashMap::new(),
            },
        );
    } else if (request.size > upload_config.max_file_size) || (request.size > i64::MAX as u64) {
        validation_errors.add(
            "size",
            ValidationError {
                code: Cow::from("size_too_large"),
                message: None,
                params: hashmap! {Cow::from("max_size") => json!(max(request.size, i64::MAX as u64))},
            },
        );
    }
    let size = request.size as i64;

    let upload = add_upload(
        NewUpload {
            extension: request.extension,
            size,
            post_id: request.post_id,
        },
        user,
        pool,
    )
    .await?;

    allocate_private_file(
        upload.id,
        request.extension,
        request.size,
        &upload_config.storage,
    )
    .await?;

    try_set_upload_status(upload.id, UploadStatus::Allocated, pool)
        .await?
        .unwrap();

    Ok(Json(UploadAddResponseOk { id: upload.id }))
}

#[put("/api/uploads/by-id/<id>/upload-by-chunk", data = "<data>")]
#[allow(clippy::too_many_arguments)]
pub async fn upload_upload_by_chunk_put<'r, 'a, 'b>(
    id: i64,
    data: Data<'r>,
    pool: &'a State<Pool<Postgres>>,
    user: User,
    upload_config: &'b State<UploadConfig>,
    _uploader: Uploader,
    content_range: ContentRange,
    _header_csrf: HeaderCSRF,
) -> Result<Json<UploadByChunkResponseOk>, crate::error::Error> {
    let upload = get_upload(id, pool).await?;

    if upload.post_author_username != user.username {
        return Err(crate::error::Error::AccessDenied);
    }

    try_set_upload_status_check_exists(id, UploadStatus::Writing, pool).await?;

    let (start_pos, end_post) = match content_range {
        ContentRange(Either::Left(bytes)) => {
            if bytes.complete_length != (upload.size as u64) {
                try_set_upload_status(id, UploadStatus::Allocated, pool).await?;
                return Err(crate::error::Error::InvalidContentRange);
            }
            (bytes.first_byte, bytes.last_byte)
        }
        ContentRange(Either::Right(unbound)) => (unbound.first_byte, unbound.last_byte),
    };
    let length = end_post + 1 - start_pos;
    {
        let mut data_stream = data.open(length.bytes());
        write_private_file(
            id,
            upload.extension.as_deref(),
            &mut data_stream,
            start_pos,
            &upload_config.storage,
        )
        .await?;
    }

    try_set_upload_status(id, UploadStatus::Allocated, pool).await?;

    Ok(Json(UploadByChunkResponseOk {}))
}

#[post("/api/uploads/by-id/<id>/finalize")]
#[allow(clippy::too_many_arguments)]
pub async fn upload_finalize_post<'r, 'a, 'b>(
    id: i64,
    pool: &'a State<Pool<Postgres>>,
    user: User,
    upload_config: &'b State<UploadConfig>,
    _uploader: Uploader,
    _header_csrf: HeaderCSRF,
) -> Result<Json<UploadByChunkResponseOk>, crate::error::Error> {
    let upload = get_upload(id, pool).await?;

    if upload.post_author_username != user.username {
        return Err(crate::error::Error::AccessDenied);
    }

    try_set_upload_status_check_exists(id, UploadStatus::Publishing, pool).await?;

    publish_file(id, upload.extension.as_deref(), &upload_config.storage).await?;

    try_set_upload_status(id, UploadStatus::Published, pool).await?;

    Ok(Json(UploadByChunkResponseOk {}))
}

#[post("/api/uploads/by-id/<id>/remove")]
#[allow(clippy::too_many_arguments)]
pub async fn upload_hide_post<'r, 'a, 'b>(
    id: i64,
    pool: &'a State<Pool<Postgres>>,
    user: User,
    upload_config: &'b State<UploadConfig>,
    _uploader: Uploader,
    _header_csrf: HeaderCSRF,
) -> Result<Json<UploadByChunkResponseOk>, crate::error::Error> {
    let upload = get_upload(id, pool).await?;

    if upload.post_author_username != user.username {
        return Err(crate::error::Error::AccessDenied);
    }

    try_set_upload_status_check_exists(id, UploadStatus::Hiding, pool).await?;

    unpublish_file(id, upload.extension.as_deref(), &upload_config.storage).await?;

    try_set_upload_status(id, UploadStatus::Hidden, pool).await?;

    Ok(Json(UploadByChunkResponseOk {}))
}
