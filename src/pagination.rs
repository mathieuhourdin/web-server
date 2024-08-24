use serde::{Deserialize};

#[derive(Deserialize)]
pub struct PaginationParams {
    offset: Option<i64>,
    limit: Option<i64>
}

impl PaginationParams {
    pub fn offset(&self) -> i64 {
        self.offset.unwrap_or(0)
    }

    pub fn limit(&self) -> i64 {
        self.limit.unwrap_or(20)
    }
}
