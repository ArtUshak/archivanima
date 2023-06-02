use std::path::Path;

use time::{Date, OffsetDateTime, PrimitiveDateTime, Time};
use tokio::fs::remove_file;

pub mod breadcrumbs;
pub mod content_range;
pub mod csrf;
pub mod csrf_lib;
pub mod form_definition;
pub mod form_extra_validation;
pub mod iter_group;
pub mod page_stream;
pub mod pagination;
pub mod template_with_status;
pub mod url_query;

pub fn date_to_offset_date_time(date: Date) -> OffsetDateTime {
    PrimitiveDateTime::new(date, Time::MIDNIGHT).assume_utc()
}

pub async fn try_remove_file(path: impl AsRef<Path>) -> std::io::Result<()> {
    match remove_file(path).await {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}
