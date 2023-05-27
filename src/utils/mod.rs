use time::{Date, OffsetDateTime, PrimitiveDateTime, Time};

pub mod breadcrumbs;
pub mod content_range;
pub mod csrf;
pub mod csrf_lib;
pub mod form_definition;
pub mod form_extra_validation;
pub mod iter_group;
pub mod pagination;
pub mod template_with_status;
pub mod url_query;

pub fn date_to_offset_date_time(date: Date) -> OffsetDateTime {
    PrimitiveDateTime::new(date, Time::MIDNIGHT).assume_utc()
}
