use std::path::PathBuf;

use artushak_web_assets::{asset_config::AssetConfig, load_cache_manifest};
use rocket::{
    fs::FileServer,
    http::{ContentType, Status},
    local::asynchronous::Client,
};
use sqlx::PgPool;
use tempfile::TempDir;
use tokio::{
    fs::{create_dir, try_exists},
    task::spawn_blocking,
};

use crate::{
    app::templates::AssetContext, asset_filters::AssetFilterCustomError, mount_views,
    run_pack_with_paths, utils::csrf_lib, PaginationConfig, UploadConfig, UploadStorage,
};

async fn initialize_rocket(pool: PgPool) -> (Client, TempDir) {
    env_logger::builder().is_test(true).init();

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
    // TODO!: static

    let rocket = mount_views(rocket);

    (Client::tracked(rocket).await.unwrap(), temp_directory)
}

#[sqlx::test(migrations = "./migrations")]
async fn test_index(pool: PgPool) {
    let (client, _) = initialize_rocket(pool).await;

    let response_index = client.get("/").dispatch().await;
    assert_eq!(response_index.status(), Status::Ok);
    assert_eq!(response_index.content_type(), Some(ContentType::HTML));
}
