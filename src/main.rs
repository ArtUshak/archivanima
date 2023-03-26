#![feature(int_roundings)]

use std::{collections::HashMap, path::PathBuf};

use app::{
    db::{try_add_user_check_username, NewUser},
    templates::AssetContext,
};
use artushak_web_assets::{
    asset_config::AssetConfig,
    asset_filter::{AssetFilter, AssetFilterRegistry},
    assets::AssetError,
    load_cache_manifest,
};
use asset_filters::{
    run_executable::AssetFilterRunExecutable, scss2css::AssetFilterSCSS, AssetFilterCustomError,
};
use clap::{Parser, Subcommand};
use log::info;
use rocket::{fs::FileServer, routes, Build, Rocket};
use rpassword::prompt_password;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use tokio::runtime::Runtime;
use utils::csrf_lib;

mod app;
mod asset_filters;
mod auth;
mod error;
mod utils;

// TODO: post moderation
// TODO: resources
// TODO: password change
// TODO: proper UX/UI
// TODO: API
// TODO: CDN
// TODO: i18n
// TOOD: proper navigation

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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaginationConfig {
    max_page_size: u64,
    default_page_size: u64,
}

pub async fn run(rocket: Rocket<Build>, config: Config) -> Result<(), error::Error> {
    let asset_cache =
        load_cache_manifest::<AssetFilterCustomError>(&config.asset_cache_manifest_path).unwrap();

    let pool = PgPoolOptions::new()
        .max_connections(config.max_db_connections)
        .connect(&config.db_url)
        .await?;

    let asset_context = AssetContext {
        asset_cache,
        base_url: config.asset_base_url.clone(),
    };

    let rocket = rocket
        .attach(csrf_lib::Fairing::default())
        .mount(
            "/",
            routes![
                app::views::index_get,
                app::views::registration_get,
                app::views::registration_post,
                app::views::login_get,
                app::views::login_post,
                app::views::logout_get,
                app::views::logout_post,
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
                app::views::post_add_post,
                app::views::post_edit_get,
                app::views::post_edit_post,
            ],
        )
        .manage(pool)
        .manage(asset_context)
        .manage(config.pagination_config);

    let rocket = if config.serve_assets {
        rocket.mount(
            &config.asset_base_url,
            FileServer::from(config.asset_config.target_directory_path),
        )
    } else {
        rocket
    };

    let _ = rocket.launch().await?;

    Ok(())
}

pub async fn run_add_user(
    config: Config,
    username: String,
    password: String,
    is_uploader: bool,
    is_admin: bool,
) -> Result<(), error::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(config.max_db_connections)
        .connect(&config.db_url)
        .await?;

    match try_add_user_check_username(
        NewUser {
            username: &username,
            password: &password,
            is_uploader,
            is_admin,
            is_active: true,
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

pub fn run_pack(config: Config) -> Result<(), AssetError<AssetFilterCustomError>> {
    let mut asset_filters: HashMap<String, Box<dyn AssetFilter<AssetFilterCustomError>>> =
        HashMap::new();
    asset_filters.insert(
        "SCSS".to_string(),
        Box::new(AssetFilterSCSS {
            format: rsass::output::Format {
                style: rsass::output::Style::Compressed,
                precision: 6,
            },
        }), // TODO
    );
    asset_filters.insert(
        "Executable".to_string(),
        Box::new(AssetFilterRunExecutable {}), // TODO
    );

    artushak_web_assets::pack(
        &config.asset_manifest_path,
        &config.asset_cache_manifest_path,
        &config.asset_config,
        &AssetFilterRegistry::new(asset_filters),
    )
}

pub fn main() {
    let opts = CLIOptions::parse();

    dotenv::dotenv().unwrap();

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
    }
}
