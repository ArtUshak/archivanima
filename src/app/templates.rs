use archivanima_macros::TemplateWithQuery;
use artushak_web_assets::asset_cache::AssetCacheManifest;
use askama::Template;
use rocket::uri;

use crate::{
    app::db::{BanReason, Post, PostVisibility, User},
    auth::Authentication,
    utils::{
        breadcrumbs::Breadcrumb, form_definition::FormDefinition, pagination::Page,
        url_query::UrlQuery,
    },
    UploadStorage,
};

pub trait TemplateWithQuery {
    fn query(&self) -> Option<&str>;
}

#[derive(Clone, Debug)]
pub struct AssetContext {
    pub asset_cache: AssetCacheManifest,
    pub base_url: String,
}

#[derive(TemplateWithQuery, Template)]
#[template(path = "index.html")]
pub struct IndexTemplate<'a> {
    pub user: Authentication,
    pub asset_context: &'a AssetContext,
    pub breadcrumbs: Vec<Breadcrumb>,
}

#[derive(TemplateWithQuery, Template)]
#[template(path = "form.html")]
pub struct FormTemplate<'a> {
    pub user: Authentication,
    pub asset_context: &'a AssetContext,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub form: FormDefinition,
}

#[derive(TemplateWithQuery, Template)]
#[template(path = "users/detail.html")]
pub struct UserDetailTemplate<'a> {
    pub user: Authentication,
    pub asset_context: &'a AssetContext,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub item: User,
}

#[derive(TemplateWithQuery, Template)]
#[template(path = "users/list.html")]
pub struct UsersListTemplate<'a> {
    pub user: Authentication,
    pub asset_context: &'a AssetContext,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub page: Page<User>,
    pub page_base: UrlQuery,
}

#[derive(TemplateWithQuery, Template)]
#[template(path = "ban-reasons/list.html")]
pub struct BanReasonListTemplate<'a> {
    pub user: Authentication,
    pub asset_context: &'a AssetContext,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub items: Vec<BanReason>,
}

#[derive(TemplateWithQuery, Template)]
#[template(path = "posts/list.html")]
pub struct PostsListTemplate<'a, 'b> {
    pub user: Authentication,
    pub asset_context: &'a AssetContext,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub page: Page<(i64, PostVisibility)>,
    pub storage: &'b UploadStorage,
    pub page_base: UrlQuery,
}

#[derive(TemplateWithQuery, Template)]
#[template(path = "posts/search.html")]
pub struct PostsSearchTemplate<'a, 'b> {
    pub user: Authentication,
    pub asset_context: &'a AssetContext,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub query_string: Option<String>,
    pub page: Page<(i64, PostVisibility)>,
    pub storage: &'b UploadStorage,
    pub page_base: UrlQuery,
}

#[derive(TemplateWithQuery, Template)]
#[template(path = "posts/detail.html")]
pub struct PostDetailTemplate<'a, 'b> {
    pub user: Authentication,
    pub asset_context: &'a AssetContext,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub item: Post,
    pub storage: &'b UploadStorage,
}

#[derive(TemplateWithQuery, Template)]
#[template(path = "posts/detail-hidden.html")]
pub struct PostDetailTemplateHidden<'a> {
    pub user: Authentication,
    pub asset_context: &'a AssetContext,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub item_id: i64,
}

#[derive(TemplateWithQuery, Template)]
#[template(path = "posts/detail-age-restricted.html")]
pub struct PostDetailTemplateAgeRestricted<'a> {
    pub user: Authentication,
    pub asset_context: &'a AssetContext,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub item_id: i64,
    pub min_age: i32,
}

#[derive(TemplateWithQuery, Template)]
#[template(path = "posts/detail-banned.html")]
pub struct PostDetailTemplateBanned<'a> {
    pub user: Authentication,
    pub asset_context: &'a AssetContext,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub item_id: i64,
    pub ban_reason: Option<BanReason>,
    pub ban_reason_text: Option<String>,
}

#[derive(TemplateWithQuery, Template)]
#[template(path = "posts/add.html")]
pub struct PostAddTemplate<'a> {
    pub user: Authentication,
    pub asset_context: &'a AssetContext,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub csrf_token: String,
}

#[derive(TemplateWithQuery, Template)]
#[template(path = "posts/edit.html")]
pub struct PostEditTemplate<'a, 'b> {
    pub user: Authentication,
    pub asset_context: &'a AssetContext,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub csrf_token: String,
    pub item: Post,
    pub storage: &'b UploadStorage,
}

mod filters {
    use std::fmt::Display;

    use crate::utils::url_query::UrlQuery;

    use super::AssetContext;

    #[derive(Clone, Debug)]
    pub struct AssetNotFoundError {
        pub asset_name: String,
    }

    impl Display for AssetNotFoundError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "asset {} not found", self.asset_name)
        }
    }

    impl std::error::Error for AssetNotFoundError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            None
        }

        fn description(&self) -> &str {
            "description() is deprecated; use Display"
        }

        fn cause(&self) -> Option<&dyn std::error::Error> {
            self.source()
        }
    }

    impl From<AssetNotFoundError> for askama::Error {
        fn from(value: AssetNotFoundError) -> Self {
            Self::Custom(Box::new(value))
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct InvalidPathError {
        pub asset_name: String,
    }

    impl Display for InvalidPathError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "invalid path of asset {}", self.asset_name)
        }
    }

    impl std::error::Error for InvalidPathError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            None
        }

        fn description(&self) -> &str {
            "description() is deprecated; use Display"
        }

        fn cause(&self) -> Option<&dyn std::error::Error> {
            self.source()
        }
    }

    impl From<InvalidPathError> for askama::Error {
        fn from(value: InvalidPathError) -> Self {
            Self::Custom(Box::new(value))
        }
    }

    pub fn load_asset(asset_context: &AssetContext, asset_name: &str) -> ::askama::Result<String> {
        Ok(asset_context
            .asset_cache
            .get_entry(asset_name)
            .ok_or_else(|| AssetNotFoundError {
                asset_name: asset_name.to_string(),
            })
            .map(|entry| {
                entry
                    .path
                    .to_str()
                    .ok_or_else(|| AssetNotFoundError {
                        asset_name: asset_name.to_string(),
                    })
                    .map(|path_str| asset_context.base_url.clone() + "/" + path_str)
            })??)
    }

    pub fn unwrap_or_string(
        string_option: &Option<String>,
        default_string: &str,
    ) -> ::askama::Result<String> {
        Ok(string_option.clone().unwrap_or(default_string.to_string()))
    }

    pub fn strip_suffix(input: &str) -> ::askama::Result<String> {
        Ok(match input.rfind('.') {
            Some(suffix_start_pos) => input[0..suffix_start_pos].to_string(),
            None => input.to_string(),
        })
    }

    pub fn url_with_pagination(
        url: &UrlQuery,
        page_id: &u64,
        page_size: &u64,
    ) -> ::askama::Result<String> {
        let mut url_copy = url.clone();
        url_copy.add("page_id".to_string(), page_id.to_string());
        url_copy.add("page_size".to_string(), page_size.to_string());
        ::askama::Result::Ok(url_copy.to_string())
    }
}
