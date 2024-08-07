#![feature(int_roundings)]
#![feature(let_chains)]
#![feature(async_closure)]
#![feature(async_fn_traits)]

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};

use app::db::set_uploads_hidden;
use artushak_web_assets::{
    asset_config::AssetConfig,
    asset_filter::{AssetFilter, AssetFilterRegistry},
    assets::AssetError,
    load_cache_manifest,
};
use asset_filters::{
    run_executable::AssetFilterRunExecutable, tsc::AssetFilterTsc, AssetFilterCustomError,
};
use clap::{Parser, Subcommand};
use log::info;
use rocket::{fs::FileServer, routes, Build, Rocket};
use rpassword::prompt_password;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tokio::runtime::Runtime;
use tokio_stream::StreamExt;
use utils::csrf_lib;

use crate::{
    app::{
        db::{list_old_in_progress_uploads_and_set_hiding, try_add_user_check_username, NewUser},
        storage::unpublish_file,
        templates::AssetContext,
    },
    utils::page_stream::iterate_pages,
};

mod app;
mod asset_filters;
mod auth;
mod error;
#[cfg(test)]
mod test;
mod utils;

#[derive(Clone, Debug, Parser)]
struct CLIOptions {
    #[clap(subcommand)]
    subcmd: CLISubcommand,
}

#[derive(Clone, Debug, Subcommand)]
enum CLISubcommand {
    Run,
    Pack,
    AddUser {
        #[arg(long)]
        username: String,
        #[arg(long)]
        is_uploader: bool,
        #[arg(long)]
        is_admin: bool,
    },
    CleanupStorage {
        #[arg(long)]
        page_size: u64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    asset_cache_manifest_path: PathBuf,
    asset_manifest_path: PathBuf,
    asset_base_url: String,
    asset_config: AssetConfig,
    serve_assets: bool,
    db_url: String,
    max_db_connections: u32,
    pagination_config: PaginationConfig,
    upload_config: UploadConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaginationConfig {
    pub max_page_size: u64,
    pub default_page_size: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UploadConfig {
    pub max_file_size: u64,
    pub storage: UploadStorage,
    pub max_upload_time: Duration,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum UploadStorage {
    FileSystem {
        private_path: PathBuf,
        public_path: PathBuf,
        base_url: String,
    },
}

pub async fn run(rocket: Rocket<Build>, config: Config) -> Result<(), error::Error> {
    let asset_cache =
        load_cache_manifest::<AssetFilterCustomError>(&config.asset_cache_manifest_path).unwrap();

    let pool = get_pool(&config).await?;

    let asset_context = AssetContext {
        asset_cache,
        base_url: config.asset_base_url.clone(),
    };

    let rocket = rocket
        .attach(csrf_lib::Fairing::default())
        .manage(pool)
        .manage(asset_context)
        .manage(config.pagination_config)
        .manage(config.upload_config.clone());

    let rocket = if config.serve_assets {
        let rocket = rocket.mount(
            &config.asset_base_url,
            FileServer::from(config.asset_config.target_directory_path),
        );
        match config.upload_config.storage {
            UploadStorage::FileSystem {
                private_path: _,
                public_path,
                base_url,
            } => rocket.mount(base_url, FileServer::from(public_path)),
        }
    } else {
        rocket
    };

    let rocket = mount_views(rocket);

    info!("Rocket configured");

    let _ = rocket.launch().await?;

    Ok(())
}

fn mount_views(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount(
        "/",
        routes![
            app::views::index_get,
            app::views::registration_get,
            app::views::registration_post,
            app::views::login_get,
            app::views::login_post,
            app::views::logout_get,
            app::views::logout_post,
            app::views::change_password_get,
            app::views::change_password_post,
            app::views::user_detail_get,
            app::views::user_edit_get,
            app::views::user_edit_post,
            app::views::ban_reasons_list_get,
            app::views::invite_add_get,
            app::views::invite_add_post,
            app::views::invite_remove_get,
            app::views::invite_remove_post,
            app::views::ban_reason_add_get,
            app::views::ban_reason_add_post,
            app::views::ban_reason_edit_get,
            app::views::ban_reason_edit_post,
            app::views::posts_list_get,
            app::views::post_detail_get,
            app::views::post_add_get,
            app::views::post_ban_get,
            app::views::post_ban_post,
            app::views::post_unban_get,
            app::views::post_unban_post,
            app::views::post_edit_get,
            app::views::posts_search_get,
            app::views::users_list_get,
            app::api::post_add_post,
            app::api::post_edit_post,
            app::api::upload_add_post,
            app::api::upload_upload_by_chunk_put,
            app::api::upload_finalize_post,
            app::api::upload_hide_post,
        ],
    )
}

pub async fn run_add_user(
    config: Config,
    username: String,
    password: String,
    is_uploader: bool,
    is_admin: bool,
) -> Result<(), error::Error> {
    let pool = get_pool(&config).await?;

    match try_add_user_check_username(
        NewUser {
            username: &username,
            password: &password,
            is_uploader,
            is_admin,
            is_active: true,
            birth_date: None,
        },
        &pool,
    )
    .await?
    {
        Some(()) => info!("User successfully created"),
        None => log::error!("Username {} already exists", username),
    }

    Ok(())
}

pub async fn run_cleanup_storage_with_pool(
    pool: &PgPool,
    storage: &UploadStorage,
    page_size: u64,
    max_age: Duration,
) -> Result<(), error::Error> {
    let mut stream = Box::pin(iterate_pages(
        page_size,
        Box::pin(async |page_params| {
            list_old_in_progress_uploads_and_set_hiding(pool, page_params, max_age).await
        }),
    ));
    while let Some(page) = stream.next().await {
        let items = page?.items;
        for upload in items.iter() {
            info!("Cleaning up file {}", upload.id);
            unpublish_file(upload.id, upload.extension.as_deref(), storage).await?;
        }
        set_uploads_hidden(pool, items.iter().map(|upload| upload.id).collect()).await?;
    }

    Ok(())
}

pub async fn run_cleanup_storage(config: Config, page_size: u64) -> Result<(), error::Error> {
    let pool = &get_pool(&config).await?;
    let storage = &config.upload_config.storage;

    run_cleanup_storage_with_pool(
        pool,
        storage,
        page_size,
        config.upload_config.max_upload_time,
    )
    .await?;

    Ok(())
}

pub fn run_pack_with_paths(
    asset_manifest_path: &Path,
    asset_cache_manifest_path: &Path,
    asset_config: &AssetConfig,
) -> Result<(), AssetError<AssetFilterCustomError>> {
    let mut asset_filters: HashMap<String, Box<dyn AssetFilter<AssetFilterCustomError>>> =
        HashMap::new();
    asset_filters.insert(
        "Executable".to_string(),
        Box::new(AssetFilterRunExecutable {}),
    );
    asset_filters.insert(
        "TSC".to_string(),
        Box::new(AssetFilterTsc {
            tsc_name: None,
            args: vec![
                "--module".to_string(),
                "amd".to_string(),
                "--baseUrl".to_string(),
                "static/scripts".to_string(),
                "--lib".to_string(),
                "ES2018,dom".to_string(),
                "--noImplicitAny".to_string(),
                "--noImplicitReturns".to_string(),
            ],
        }),
    );

    artushak_web_assets::pack(
        asset_manifest_path,
        asset_cache_manifest_path,
        asset_config,
        &AssetFilterRegistry::new(asset_filters),
    )
}

pub fn run_pack(config: Config) -> Result<(), AssetError<AssetFilterCustomError>> {
    run_pack_with_paths(
        &config.asset_manifest_path,
        &config.asset_cache_manifest_path,
        &config.asset_config,
    )
}

async fn get_pool(config: &Config) -> Result<PgPool, error::Error> {
    Ok(PgPoolOptions::new()
        .max_connections(config.max_db_connections)
        .connect(&config.db_url)
        .await?)
}

pub fn main() {
    let opts = CLIOptions::parse();

    env_logger::init();

    let rocket = rocket::build();
    let figment = rocket.figment();

    let config: Config = figment.extract().unwrap();

    match opts.subcmd {
        CLISubcommand::Run => {
            Runtime::new()
                .unwrap()
                .block_on(run(rocket, config))
                .unwrap();
        }
        CLISubcommand::Pack => {
            run_pack(config).unwrap();
        }
        CLISubcommand::AddUser {
            username,
            is_uploader,
            is_admin,
        } => {
            let password = prompt_password("Type password for new user: ").unwrap();
            Runtime::new()
                .unwrap()
                .block_on(run_add_user(
                    config,
                    username,
                    password,
                    is_uploader,
                    is_admin,
                ))
                .unwrap();
        }
        CLISubcommand::CleanupStorage { page_size } => {
            Runtime::new()
                .unwrap()
                .block_on(run_cleanup_storage(config, page_size))
                .unwrap();
        }
    }
}
