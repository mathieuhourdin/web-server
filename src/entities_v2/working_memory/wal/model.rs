use chrono::{NaiveDate, NaiveDateTime};
use diesel::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use crate::schema::wal_days;

#[derive(Debug, Clone, Queryable, Selectable, Identifiable)]
#[diesel(table_name = wal_days)]
pub struct WalDay {
    pub id: Uuid,
    pub user_id: Uuid,
    pub local_date: NaiveDate,
    pub input: String,
    pub context: String,
    pub compiled_operational: Option<String>,
    pub compiled_thematic: Option<String>,
    pub compiled_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = wal_days)]
pub(crate) struct NewWalDay {
    pub id: Uuid,
    pub user_id: Uuid,
    pub local_date: NaiveDate,
    pub input: String,
    pub context: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WalCompilation {
    pub content: String,
    pub compiled_at: NaiveDateTime,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct WalCompilationViews {
    pub operational: Option<WalCompilation>,
    pub thematic: Option<WalCompilation>,
}

#[derive(Debug, Serialize)]
pub struct WalResponse {
    pub id: Uuid,
    pub date: NaiveDate,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct WalDayResponse {
    pub id: Uuid,
    pub date: NaiveDate,
    pub content: String,
    pub operational: Option<WalCompilation>,
    pub thematic: Option<WalCompilation>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl WalDay {
    pub fn compilation_views(&self) -> WalCompilationViews {
        WalCompilationViews {
            operational: self
                .compiled_operational
                .as_ref()
                .zip(self.compiled_at)
                .map(|(content, compiled_at)| WalCompilation {
                    content: content.clone(),
                    compiled_at,
                }),
            thematic: self.compiled_thematic.as_ref().zip(self.compiled_at).map(
                |(content, compiled_at)| WalCompilation {
                    content: content.clone(),
                    compiled_at,
                },
            ),
        }
    }
}

impl From<&WalDay> for WalResponse {
    fn from(wal: &WalDay) -> Self {
        Self {
            id: wal.id,
            date: wal.local_date,
            content: wal.input.clone(),
        }
    }
}

impl From<WalDay> for WalDayResponse {
    fn from(wal: WalDay) -> Self {
        let views = wal.compilation_views();
        Self {
            id: wal.id,
            date: wal.local_date,
            content: wal.input,
            operational: views.operational,
            thematic: views.thematic,
            created_at: wal.created_at,
            updated_at: wal.updated_at,
        }
    }
}
