use std::{ops::AsyncFn, pin::Pin};

use async_stream::try_stream;
use tokio_stream::Stream;

use crate::utils::pagination::{Page, PageParams};

pub fn iterate_pages<T, Fun>(
    page_size: u64,
    page_func: Pin<Box<Fun>>,
) -> impl Stream<Item = Result<Page<T>, crate::error::Error>>
where
    Fun: AsyncFn(PageParams) -> Result<Page<T>, crate::error::Error>,
{
    try_stream! {
        let mut page_id = 0;
        loop {
            let page_params = PageParams {
                page_id: Some(page_id),
                page_size,
            };
            match (page_func)(page_params).await {
                Ok(page) => {
                    let page_count = page.page_count;
                    yield page;
                    if (page_id + 1) >= page_count {
                        break;
                    } else {
                        page_id += 1;
                    }
                },
                Err(crate::error::Error::PageDoesNotExist) => {
                    break;
                },
                other => {
                    other?;
                }
            }

        }
    }
}
