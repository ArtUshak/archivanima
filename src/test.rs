use std::{path::PathBuf, sync::Once, time::Duration};

use artushak_web_assets::{asset_config::AssetConfig, load_cache_manifest};
use itertools::Itertools;
use kuchikiki::{parse_html, traits::*};
use rocket::{
    fs::FileServer,
    http::{ContentType, Cookie, Header, Status},
    local::asynchronous::Client,
    serde::json::from_str,
};
use serde_json::{Map, Value};
use sqlx::PgPool;
use tempfile::TempDir;
use tokio::{
    fs::{create_dir, try_exists},
    task::spawn_blocking,
    time::sleep,
};

use crate::{
    app::{
        db::{
            add_post, add_upload, get_upload, try_add_user_check_username, try_set_upload_status,
            NewPost, NewUpload, NewUser, UploadStatus, User,
        },
        storage::{allocate_private_file, publish_file, write_private_file},
        templates::AssetContext,
    },
    asset_filters::AssetFilterCustomError,
    mount_views, run_cleanup_storage_with_pool, run_pack_with_paths,
    utils::{csrf_lib, url_query::UrlQuery},
    PaginationConfig, UploadConfig, UploadStorage,
};

static INIT: Once = Once::new();

async fn initialize_rocket(pool: PgPool) -> (Client, TempDir) {
    INIT.call_once(|| env_logger::builder().is_test(true).init()); // TODO: async

    let root_temp_directory_path: PathBuf = ".tmp".into();
    if !try_exists(&root_temp_directory_path).await.unwrap() {
        create_dir(&root_temp_directory_path).await.unwrap();
    }

    let temp_directory = spawn_blocking(move || TempDir::new_in(root_temp_directory_path))
        .await
        .unwrap()
        .unwrap();
    let temp_directory_path = temp_directory.path();

    let data_directory_path = temp_directory_path.join("data");
    create_dir(&data_directory_path).await.unwrap();

    let data_public_directory_path = temp_directory_path.join("datapublic");
    create_dir(&data_public_directory_path).await.unwrap();

    let internal_directory_path = temp_directory_path.join("internal");
    create_dir(&internal_directory_path).await.unwrap();

    let static_directory_path = temp_directory_path.join("static");
    create_dir(&static_directory_path).await.unwrap();

    let asset_manifest_path: PathBuf = "assets.json".into();
    let asset_cache_manifest_path = temp_directory_path.join("assets_cache.json");

    let asset_config = AssetConfig {
        target_directory_path: static_directory_path,
        internal_directory_path,
        source_directory_path: "static".into(),
    };

    let asset_cache_manifest_path_clone = asset_cache_manifest_path.clone();

    spawn_blocking(move || {
        run_pack_with_paths(
            &asset_manifest_path,
            &asset_cache_manifest_path_clone,
            &asset_config,
        )
    })
    .await
    .unwrap()
    .unwrap();

    let asset_cache = spawn_blocking(move || {
        load_cache_manifest::<AssetFilterCustomError>(&asset_cache_manifest_path).unwrap()
    })
    .await
    .unwrap();

    let upload_config = UploadConfig {
        max_file_size: 128 * 1024 * 1024,
        storage: UploadStorage::FileSystem {
            private_path: data_directory_path,
            public_path: data_public_directory_path.clone(),
            base_url: "/media/".to_string(),
        },
        max_upload_time: Duration::from_secs(36 * 60 * 60),
    };

    let pagination_config = PaginationConfig {
        max_page_size: 100,
        default_page_size: 10,
    };

    let rocket = rocket::build();

    let asset_context = AssetContext {
        asset_cache,
        base_url: "/static/".to_string(),
    };

    let rocket = rocket
        .attach(csrf_lib::Fairing::default())
        .manage(pool)
        .manage(asset_context)
        .manage(pagination_config)
        .manage(upload_config)
        .mount("/media/", FileServer::from(data_public_directory_path));
    // TODO: static

    let rocket = mount_views(rocket);

    (Client::tracked(rocket).await.unwrap(), temp_directory)
}

#[sqlx::test(migrations = "./migrations")]
async fn test_index(pool: PgPool) {
    let (client, _temp_dir) = initialize_rocket(pool).await;

    let response_index = client.get("/").dispatch().await;
    assert_eq!(response_index.status(), Status::Ok);
    assert_eq!(response_index.content_type(), Some(ContentType::HTML));
}

async fn try_login<'a, 'b>(
    client: &Client,
    username: &str,
    password: &str,
    cookies: Option<Vec<Cookie<'a>>>,
) -> Option<Vec<Cookie<'b>>> {
    let request = client.get("/auth/login");
    let request = if let Some(cookies) = cookies {
        request.cookies(cookies)
    } else {
        request
    };
    let response = request.dispatch().await;
    let cookies: Vec<_> = response.cookies().iter().cloned().collect();
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::HTML));
    let response_text = response.into_string().await.unwrap();
    let document = parse_html().one(response_text.as_str());
    let document_forms: Vec<_> = document.select("main form").unwrap().collect();
    assert_eq!(document_forms.len(), 1);
    let document_form = document_forms.first().unwrap();
    let input_csrf: Vec<_> = document_form
        .as_node()
        .select("input[name=csrf_token]")
        .unwrap()
        .collect();
    assert_eq!(input_csrf.len(), 1);
    let csrf = input_csrf
        .first()
        .unwrap()
        .as_node()
        .as_element()
        .unwrap()
        .attributes
        .borrow()
        .get("value")
        .unwrap()
        .to_string();

    let request_form = {
        let mut request_form = UrlQuery::new();
        request_form.add("csrf_token".to_string(), csrf.to_string());
        request_form.add("username".to_string(), username.to_string());
        request_form.add("password".to_string(), password.to_string());
        request_form
    };

    let response = client
        .post("/auth/login")
        .header(ContentType::Form)
        .body(request_form.to_string())
        .cookies(cookies)
        .dispatch()
        .await;
    let cookies: Vec<_> = response.cookies().iter().cloned().collect();
    let status = response.status();
    if status == Status::SeeOther {
        assert_eq!(response.headers().get_one("location"), Some("/"));
        Some(cookies)
    } else if status == Status::Ok {
        None
    } else {
        panic!("login request returned status {}", status);
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_login(pool: PgPool) {
    try_add_user_check_username(
        NewUser {
            username: "admin1",
            password: "password1",
            is_active: true,
            is_admin: true,
            is_uploader: false,
            birth_date: None,
        },
        &pool,
    )
    .await
    .unwrap();

    let (client, _temp_dir) = initialize_rocket(pool).await;
    let auth_result = try_login(&client, "admin1", "password1", None).await;
    let cookies = auth_result.unwrap();

    let response = client.get("/").cookies(cookies).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::HTML));
    let response_text = response.into_string().await.unwrap();
    let document = parse_html().one(response_text.as_str());
    let document_nav_lines: Vec<_> = document.select("header > nav > ul > li").unwrap().collect();
    let document_nav_line = document_nav_lines.first().unwrap();
    let document_nav_line_text = document_nav_line.text_contents();
    assert!(document_nav_line_text.starts_with("ты admin1"));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_fail_login(pool: PgPool) {
    try_add_user_check_username(
        NewUser {
            username: "admin1",
            password: "password1",
            is_active: true,
            is_admin: true,
            is_uploader: false,
            birth_date: None,
        },
        &pool,
    )
    .await
    .unwrap();

    let (client, _temp_dir) = initialize_rocket(pool).await;
    let auth_result = try_login(&client, "admin1", "password2", None).await;
    assert_eq!(auth_result, None);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_add_post(pool: PgPool) {
    try_add_user_check_username(
        NewUser {
            username: "admin1",
            password: "password1",
            is_active: true,
            is_admin: true,
            is_uploader: false,
            birth_date: None,
        },
        &pool,
    )
    .await
    .unwrap();

    let (client, _temp_dir) = initialize_rocket(pool).await;

    let auth_result = try_login(&client, "admin1", "password1", None).await;
    let cookies = auth_result.unwrap();

    let response = client.get("/posts/add").cookies(cookies).dispatch().await;
    let cookies: Vec<_> = response.cookies().iter().cloned().collect();
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::HTML));
    let response_text = response.into_string().await.unwrap();
    let document = parse_html().one(response_text.as_str());
    let document_metas_csrf: Vec<_> = document
        .select("meta[name=\"csrf-token\"]")
        .unwrap()
        .collect();
    let document_meta_csrf = document_metas_csrf.first().unwrap();
    let csrf = document_meta_csrf
        .as_node()
        .as_element()
        .unwrap()
        .attributes
        .borrow()
        .get("content")
        .unwrap()
        .to_string();

    let request_body = r#"{
        "title": "осторожно, метамодерн!",
        "description": "пилотный выпуск нового шоу",
        "is_hidden": false,
        "is_pinned": false,
        "min_age": null
    }"#;
    let response = client
        .post("/api/posts/add")
        .cookies(cookies)
        .header(ContentType::JSON)
        .body(request_body)
        .header(Header::new("X-CSRF-Token", csrf))
        .dispatch()
        .await;
    let cookies: Vec<_> = response.cookies().iter().cloned().collect();
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::JSON));
    let response_text = response.into_string().await.unwrap();
    let response_data: serde_json::Map<String, serde_json::Value> =
        from_str(&response_text).unwrap();
    let response_post_url = response_data.get("url").unwrap().as_str().unwrap();
    let response_post_id = response_data.get("id").unwrap().as_i64().unwrap();

    let response = client
        .get(response_post_url)
        .cookies(cookies)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::HTML));
    let response_text = response.into_string().await.unwrap();
    let document = parse_html().one(response_text.as_str());
    let document_post_headers: Vec<_> = document.select("article > h2").unwrap().collect();
    assert_eq!(document_post_headers.len(), 1);
    let document_post_header = document_post_headers.first().unwrap();
    let document_post_header_text = document_post_header.text_contents();
    assert_eq!(
        document_post_header_text,
        format!("#{response_post_id}: осторожно, метамодерн!")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_edit_post(pool: PgPool) {
    try_add_user_check_username(
        NewUser {
            username: "admin1",
            password: "password1",
            is_active: true,
            is_admin: true,
            is_uploader: false,
            birth_date: None,
        },
        &pool,
    )
    .await
    .unwrap();

    let post = add_post(
        NewPost {
            title: "осторожно, метамодерн!",
            description:
                "пилотный выпуск нового шоу!\n\nоставляйте обратную связь на почту или в Телеграм",
            is_hidden: false,
            min_age: None,
            is_pinned: false,
        },
        User {
            username: "admin1".to_string(),
            is_active: true,
            is_admin: true,
            is_uploader: true,
            birth_date: None,
        },
        &pool,
    )
    .await
    .unwrap();

    let (client, _temp_dir) = initialize_rocket(pool).await;

    let auth_result = try_login(&client, "admin1", "password1", None).await;
    let cookies = auth_result.unwrap();

    let response = client
        .get(format!("/posts/by-id/{}/edit", post.id))
        .cookies(cookies)
        .dispatch()
        .await;
    let cookies: Vec<_> = response.cookies().iter().cloned().collect();
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::HTML));
    let response_text = response.into_string().await.unwrap();
    let document = parse_html().one(response_text.as_str());
    let document_metas_csrf: Vec<_> = document
        .select("meta[name=\"csrf-token\"]")
        .unwrap()
        .collect();
    let document_meta_csrf = document_metas_csrf.first().unwrap();
    let csrf = document_meta_csrf
        .as_node()
        .as_element()
        .unwrap()
        .attributes
        .borrow()
        .get("content")
        .unwrap()
        .to_string();

    let request_body = r#"{
        "title": "осторожно, метамодерн! (пилотный выпуск)",
        "description": null,
        "is_hidden": null,
        "min_age": null
    }"#;
    let response = client
        .post(format!("/api/posts/by-id/{}/edit", post.id))
        .cookies(cookies)
        .header(ContentType::JSON)
        .body(request_body)
        .header(Header::new("X-CSRF-Token", csrf))
        .dispatch()
        .await;
    let cookies: Vec<_> = response.cookies().iter().cloned().collect();
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::JSON));
    let response_text = response.into_string().await.unwrap();
    let _response_data: serde_json::Map<String, serde_json::Value> =
        from_str(&response_text).unwrap();

    let response = client
        .get(format!("/posts/by-id/{}", post.id))
        .cookies(cookies)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::HTML));
    let response_text = response.into_string().await.unwrap();
    let document = parse_html().one(response_text.as_str());
    let document_post_headers: Vec<_> = document.select("article > h2").unwrap().collect();
    assert_eq!(document_post_headers.len(), 1);
    let document_post_header = document_post_headers.first().unwrap();
    let document_post_header_text = document_post_header.text_contents();
    assert_eq!(
        document_post_header_text,
        format!("#{}: осторожно, метамодерн! (пилотный выпуск)", post.id)
    );
    let document_post_paragraphs: Vec<_> = document
        .select("article > p.post-creation-date ~ p")
        .unwrap()
        .collect();
    assert_eq!(document_post_paragraphs.len(), 2);
    let document_post_paragraphs_text = document_post_paragraphs
        .iter()
        .map(|element| element.text_contents())
        .join("\n");
    assert_eq!(
        document_post_paragraphs_text,
        "пилотный выпуск нового шоу!\nоставляйте обратную связь на почту или в Телеграм"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_add_upload(pool: PgPool) {
    try_add_user_check_username(
        NewUser {
            username: "admin1",
            password: "password1",
            is_active: true,
            is_admin: true,
            is_uploader: false,
            birth_date: None,
        },
        &pool,
    )
    .await
    .unwrap();

    let post = add_post(
        NewPost {
            title: "осторожно, метамодерн! (пилотный выпуск)",
            description:
                "пилотный выпуск нового шоу!\n\nоставляйте обратную связь на почту или в Телеграм",
            is_hidden: false,
            min_age: None,
            is_pinned: false,
        },
        User {
            username: "admin1".to_string(),
            is_active: true,
            is_admin: true,
            is_uploader: true,
            birth_date: None,
        },
        &pool,
    )
    .await
    .unwrap();

    let (client, _temp_dir) = initialize_rocket(pool).await;

    let auth_result = try_login(&client, "admin1", "password1", None).await;
    let cookies = auth_result.unwrap();

    let response = client
        .get(format!("/posts/by-id/{}/edit", post.id))
        .cookies(cookies)
        .dispatch()
        .await;
    let cookies: Vec<_> = response.cookies().iter().cloned().collect();
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::HTML));
    let response_text = response.into_string().await.unwrap();
    let document = parse_html().one(response_text.as_str());
    let document_metas_csrf: Vec<_> = document
        .select("meta[name=\"csrf-token\"]")
        .unwrap()
        .collect();
    let document_meta_csrf = document_metas_csrf.first().unwrap();
    let csrf = document_meta_csrf
        .as_node()
        .as_element()
        .unwrap()
        .attributes
        .borrow()
        .get("content")
        .unwrap()
        .to_string();

    let upload_content = b"THIS IS TEST FILE!\nTHANK YOU FOR YOUR ATTENTION.\n";
    let upload_content_size = upload_content.len();

    let request_data: Map<String, Value> = Map::from_iter(vec![
        (
            "size".to_string(),
            Value::Number(upload_content_size.into()),
        ),
        ("post_id".to_string(), Value::Number(post.id.into())),
        ("extension".to_string(), Value::String("txt".to_string())),
    ]);
    let request_body = serde_json::to_string(&request_data).unwrap();
    let response = client
        .post(format!("/api/uploads/add",))
        .cookies(cookies)
        .header(ContentType::JSON)
        .body(request_body)
        .header(Header::new("X-CSRF-Token", csrf.clone()))
        .dispatch()
        .await;
    let cookies: Vec<_> = response.cookies().iter().cloned().collect();
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::JSON));
    let response_text = response.into_string().await.unwrap();
    let response_data: serde_json::Map<String, serde_json::Value> =
        from_str(&response_text).unwrap();
    let response_upload_id = response_data.get("id").unwrap().as_i64().unwrap();

    let response = client
        .put(format!(
            "/api/uploads/by-id/{}/upload-by-chunk",
            response_upload_id
        ))
        .header(Header::new("X-CSRF-Token", csrf.clone()))
        .header(Header::new(
            "content-range",
            format!(
                "bytes {}-{}/{}",
                0,
                upload_content_size - 1,
                upload_content_size
            ),
        ))
        .cookies(cookies)
        .body(upload_content)
        .dispatch()
        .await;
    let cookies: Vec<_> = response.cookies().iter().cloned().collect();
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::JSON));
    let response_text = response.into_string().await.unwrap();
    let _response_data: serde_json::Map<String, serde_json::Value> =
        from_str(&response_text).unwrap();

    let response = client
        .get(format!("/posts/by-id/{}", post.id))
        .cookies(cookies)
        .dispatch()
        .await;
    let cookies: Vec<_> = response.cookies().iter().cloned().collect();
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::HTML));
    let response_text = response.into_string().await.unwrap();
    let document = parse_html().one(response_text.as_str());
    let document_post_headers: Vec<_> = document.select("article > h2").unwrap().collect();
    assert_eq!(document_post_headers.len(), 1);
    let document_post_header = document_post_headers.first().unwrap();
    let document_post_header_text = document_post_header.text_contents();
    assert_eq!(
        document_post_header_text,
        format!("#{}: осторожно, метамодерн! (пилотный выпуск)", post.id)
    );
    let document_post_attachment_links: Vec<_> = document
        .select("article > p.post.attachments > li > a")
        .unwrap()
        .collect();
    assert_eq!(document_post_attachment_links.len(), 0);

    let response = client
        .post(format!(
            "/api/uploads/by-id/{}/finalize",
            response_upload_id
        ))
        .header(Header::new("X-CSRF-Token", csrf))
        .cookies(cookies)
        .dispatch()
        .await;
    let cookies: Vec<_> = response.cookies().iter().cloned().collect();
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::JSON));
    let response_text = response.into_string().await.unwrap();
    let _response_data: serde_json::Map<String, serde_json::Value> =
        from_str(&response_text).unwrap();

    let response = client
        .get(format!("/posts/by-id/{}", post.id))
        .cookies(cookies)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::HTML));
    let response_text = response.into_string().await.unwrap();
    let document = parse_html().one(response_text.as_str());
    let document_post_headers: Vec<_> = document.select("article > h2").unwrap().collect();
    assert_eq!(document_post_headers.len(), 1);
    let document_post_attachment_links: Vec<_> = document
        .select("article > ul.post-attachments > li > a")
        .unwrap()
        .collect();
    assert_eq!(document_post_attachment_links.len(), 1);

    let document_post_attachment_link = document_post_attachment_links[0]
        .attributes
        .borrow()
        .get("href")
        .unwrap()
        .to_string();

    let attachment_response = client.get(document_post_attachment_link).dispatch().await;
    assert_eq!(attachment_response.status(), Status::Ok);
    let attachment_response_bytes = attachment_response.into_bytes().await.unwrap();
    assert_eq!(attachment_response_bytes, upload_content);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_cleanup_uploads(pool: PgPool) {
    let root_temp_directory_path: PathBuf = ".tmp".into();
    if !try_exists(&root_temp_directory_path).await.unwrap() {
        create_dir(&root_temp_directory_path).await.unwrap();
    }

    let temp_directory = spawn_blocking(move || TempDir::new_in(root_temp_directory_path))
        .await
        .unwrap()
        .unwrap();
    let temp_directory_path = temp_directory.path();

    let data_directory_path = temp_directory_path.join("data");
    create_dir(&data_directory_path).await.unwrap();

    let data_public_directory_path = temp_directory_path.join("datapublic");
    create_dir(&data_public_directory_path).await.unwrap();

    let upload_config = UploadConfig {
        max_file_size: 128 * 1024 * 1024,
        storage: UploadStorage::FileSystem {
            private_path: data_directory_path,
            public_path: data_public_directory_path.clone(),
            base_url: "/media/".to_string(),
        },
        max_upload_time: Duration::from_secs(0),
    };

    try_add_user_check_username(
        NewUser {
            username: "admin1",
            password: "password1",
            is_active: true,
            is_admin: true,
            is_uploader: false,
            birth_date: None,
        },
        &pool,
    )
    .await
    .unwrap();

    let post = add_post(
        NewPost {
            title: "осторожно, метамодерн! (пилотный выпуск)",
            description:
                "пилотный выпуск нового шоу!\n\nоставляйте обратную связь на почту или в Телеграм",
            is_hidden: false,
            min_age: None,
            is_pinned: false,
        },
        User {
            username: "admin1".to_string(),
            is_active: true,
            is_admin: true,
            is_uploader: true,
            birth_date: None,
        },
        &pool,
    )
    .await
    .unwrap();

    let upload_content = b"THIS IS TEST FILE!\nTHANK YOU FOR YOUR ATTENTION.\n".as_slice();
    let upload_content_size = upload_content.len();

    let upload1 = add_upload(
        NewUpload {
            extension: Some("txt"),
            size: upload_content_size as i64,
            post_id: post.id,
        },
        User {
            username: "admin1".to_string(),
            is_active: true,
            is_admin: true,
            is_uploader: false,
            birth_date: None,
        },
        &pool,
    )
    .await
    .unwrap();

    allocate_private_file(
        upload1.id,
        upload1.extension.as_deref(),
        upload_content_size as u64,
        &upload_config.storage,
    )
    .await
    .unwrap();
    try_set_upload_status(upload1.id, UploadStatus::Allocated, &pool)
        .await
        .unwrap();
    try_set_upload_status(upload1.id, UploadStatus::Writing, &pool)
        .await
        .unwrap();
    let mut upload_content_copy = upload_content;
    write_private_file(
        upload1.id,
        upload1.extension.as_deref(),
        &mut upload_content_copy,
        0,
        &upload_config.storage,
    )
    .await
    .unwrap();
    try_set_upload_status(upload1.id, UploadStatus::Allocated, &pool)
        .await
        .unwrap();
    try_set_upload_status(upload1.id, UploadStatus::Publishing, &pool)
        .await
        .unwrap();
    publish_file(
        upload1.id,
        upload1.extension.as_deref(),
        &upload_config.storage,
    )
    .await
    .unwrap();
    try_set_upload_status(upload1.id, UploadStatus::Published, &pool)
        .await
        .unwrap();

    let upload2 = add_upload(
        NewUpload {
            extension: Some("txt"),
            size: upload_content_size as i64,
            post_id: post.id,
        },
        User {
            username: "admin1".to_string(),
            is_active: true,
            is_admin: true,
            is_uploader: false,
            birth_date: None,
        },
        &pool,
    )
    .await
    .unwrap();

    allocate_private_file(
        upload2.id,
        upload2.extension.as_deref(),
        upload_content_size as u64,
        &upload_config.storage,
    )
    .await
    .unwrap();
    try_set_upload_status(upload2.id, UploadStatus::Allocated, &pool)
        .await
        .unwrap();
    try_set_upload_status(upload2.id, UploadStatus::Writing, &pool)
        .await
        .unwrap();
    let mut upload_content_copy = upload_content;
    write_private_file(
        upload2.id,
        upload2.extension.as_deref(),
        &mut upload_content_copy,
        0,
        &upload_config.storage,
    )
    .await
    .unwrap();
    try_set_upload_status(upload2.id, UploadStatus::Allocated, &pool)
        .await
        .unwrap();

    let upload3 = add_upload(
        NewUpload {
            extension: Some("txt"),
            size: upload_content_size as i64,
            post_id: post.id,
        },
        User {
            username: "admin1".to_string(),
            is_active: true,
            is_admin: true,
            is_uploader: false,
            birth_date: None,
        },
        &pool,
    )
    .await
    .unwrap();

    allocate_private_file(
        upload3.id,
        upload3.extension.as_deref(),
        upload_content_size as u64,
        &upload_config.storage,
    )
    .await
    .unwrap();
    try_set_upload_status(upload3.id, UploadStatus::Allocated, &pool)
        .await
        .unwrap();
    try_set_upload_status(upload3.id, UploadStatus::Writing, &pool)
        .await
        .unwrap();
    let mut upload_content_copy = upload_content;
    write_private_file(
        upload2.id,
        upload2.extension.as_deref(),
        &mut upload_content_copy,
        0,
        &upload_config.storage,
    )
    .await
    .unwrap();

    sleep(Duration::from_millis(500)).await;

    run_cleanup_storage_with_pool(&pool, &upload_config.storage, 2, Duration::from_millis(0))
        .await
        .unwrap();

    assert_eq!(
        get_upload(upload1.id, &pool).await.unwrap().file_status,
        UploadStatus::Published
    );
    assert_eq!(
        get_upload(upload2.id, &pool).await.unwrap().file_status,
        UploadStatus::Hidden
    );
    assert_eq!(
        get_upload(upload3.id, &pool).await.unwrap().file_status,
        UploadStatus::Hidden
    );

    // TODO: check file existence
}

#[sqlx::test(migrations = "./migrations")]
async fn test_list_users(pool: PgPool) {
    try_add_user_check_username(
        NewUser {
            username: "admin1",
            password: "password1",
            is_active: true,
            is_admin: true,
            is_uploader: false,
            birth_date: None,
        },
        &pool,
    )
    .await
    .unwrap();

    let (client, _temp_dir) = initialize_rocket(pool).await;

    let auth_result = try_login(&client, "admin1", "password1", None).await;
    let cookies = auth_result.unwrap();

    let response = client
        .get(format!("/users"))
        .cookies(cookies)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::HTML));
    let response_text = response.into_string().await.unwrap();
    let document = parse_html().one(response_text.as_str());
    let document_rows: Vec<_> = document
        .select("article > div > table > tbody > tr")
        .unwrap()
        .collect();
    assert_eq!(document_rows.len(), 1);

    let document_row = document_rows.first().unwrap();

    let document_row_cells: Vec<_> = document_row.as_node().select("th, td").unwrap().collect();
    assert_eq!(document_row_cells.len(), 5);
    let document_row_cell_texts: Vec<_> = document_row_cells[0..4]
        .iter()
        .map(|cell| cell.text_contents())
        .collect();
    assert_eq!(
        document_row_cell_texts,
        vec!["admin1", "активен", "администратор", ""]
    );
}

// TODO: test permissions
// TODO: test age restriction
// TODO: test post bans
// TODO: test CSRF
// TODO: test UI by Selenium
