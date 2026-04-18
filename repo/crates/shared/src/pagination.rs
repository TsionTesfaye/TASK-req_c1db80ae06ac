//! Pagination defaults — 50 rows per page, hard max 200.

use serde::{Deserialize, Serialize};

pub const DEFAULT_PAGE_SIZE: u32 = 50;
pub const MAX_PAGE_SIZE: u32 = 200;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PageQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

impl PageQuery {
    /// Resolve defaults and clamp to [`MAX_PAGE_SIZE`].
    pub fn resolved(self) -> Resolved {
        let page = self.page.unwrap_or(1).max(1);
        let page_size = self
            .page_size
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE);
        Resolved { page, page_size }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Resolved {
    pub page: u32,
    pub page_size: u32,
}

impl Resolved {
    pub fn offset(&self) -> u64 {
        u64::from(self.page - 1) * u64::from(self.page_size)
    }
    pub fn limit(&self) -> u64 {
        u64::from(self.page_size)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_clamp() {
        let r = PageQuery {
            page: None,
            page_size: None,
        }
        .resolved();
        assert_eq!(r.page, 1);
        assert_eq!(r.page_size, DEFAULT_PAGE_SIZE);
        assert_eq!(r.offset(), 0);

        let r = PageQuery {
            page: Some(0),
            page_size: Some(9999),
        }
        .resolved();
        assert_eq!(r.page, 1);
        assert_eq!(r.page_size, MAX_PAGE_SIZE);
    }
}
