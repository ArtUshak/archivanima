use log::debug;

use crate::PaginationConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageParams {
    pub page_id: Option<u64>,
    pub page_size: u64,
}

impl PageParams {
    pub fn check(&self, config: &PaginationConfig) -> Result<(), crate::error::Error> {
        if (self.page_size == 0) || (self.page_size > config.max_page_size) {
            Err(crate::error::Error::InvalidPagination)
        } else {
            Ok(())
        }
    }

    pub fn get_limit_offset_and_page_id(
        &self,
        total_page_count: u64,
    ) -> Result<(i64, i64, u64), crate::error::Error> {
        if self.page_size == 0 {
            return Err(crate::error::Error::InvalidPagination);
        }

        let page_id = match self.page_id {
            Some(page_id) if page_id < total_page_count => Ok(page_id),
            Some(_) => Err(crate::error::Error::PageDoesNotExist),
            None if total_page_count > 0 => Ok(total_page_count - 1),
            _ => Ok(0),
        }?;

        let page_id_i64: i64 = page_id
            .try_into()
            .map_err(|_| crate::error::Error::InvalidPagination)?;

        let limit_i64 = self
            .page_size
            .try_into()
            .map_err(|_| crate::error::Error::InvalidPagination)?;

        debug!(
            "{} {:?} {}",
            limit_i64,
            page_id_i64.checked_mul(limit_i64),
            page_id
        ); // TODO!

        Ok((
            limit_i64,
            page_id_i64
                .checked_mul(limit_i64)
                .ok_or(crate::error::Error::InvalidPagination)?,
            page_id,
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub page_id: u64,
    pub page_size: u64,
    pub page_count: u64,
    pub total_item_count: u64,
}

impl<T> Page<T> {
    pub fn map<F, Y>(&self, func: F) -> Page<Y>
    where
        F: Fn(&T) -> Y,
    {
        Page {
            items: self.items.iter().map(func).collect(),
            page_id: self.page_id,
            page_size: self.page_size,
            page_count: self.page_count,
            total_item_count: self.total_item_count,
        }
    }
}
