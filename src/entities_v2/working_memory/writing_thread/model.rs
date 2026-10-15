use crate::pagination::PaginationParams;
use chrono::NaiveDateTime;
use diesel::{
    sql_types::{BigInt, Bool, Nullable, Text, Timestamp, Uuid as SqlUuid},
    QueryableByName,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, QueryableByName)]
pub struct WritingThread {
    #[diesel(sql_type=SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type=SqlUuid)]
    pub user_id: Uuid,
    #[diesel(sql_type=SqlUuid)]
    pub start_trace_id: Uuid,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    pub latest_trace_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    pub pending_trace_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<Text>)]
    pub title: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    pub ongoing_question: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    pub journey_summary: Option<String>,
    #[diesel(sql_type=diesel::sql_types::Array<Text>)]
    pub search_tags: Vec<String>,
    #[diesel(sql_type=BigInt)]
    pub input_revision: i64,
    #[diesel(sql_type=Nullable<BigInt>)]
    pub compiled_revision: Option<i64>,
    #[diesel(sql_type=Text)]
    pub generation_status: String,
    #[serde(skip_serializing)]
    #[diesel(sql_type=Nullable<SqlUuid>)]
    pub generation_claim_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<Text>)]
    pub generation_error: Option<String>,
    #[diesel(sql_type=Nullable<Timestamp>)]
    pub generated_at: Option<NaiveDateTime>,
    #[diesel(sql_type=Timestamp)]
    pub created_at: NaiveDateTime,
    #[diesel(sql_type=Timestamp)]
    pub updated_at: NaiveDateTime,
}
#[derive(Serialize, QueryableByName)]
pub struct ThreadView {
    #[diesel(embed)]
    #[serde(flatten)]
    pub thread: WritingThread,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    pub journal_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<Text>)]
    pub latest_trace_title: Option<String>,
    #[diesel(sql_type=Nullable<Timestamp>)]
    pub last_activity_at: Option<NaiveDateTime>,
    #[diesel(sql_type=BigInt)]
    pub trace_count: i64,
    #[diesel(sql_type=BigInt)]
    pub character_count: i64,
    #[diesel(sql_type=Bool)]
    pub can_continue: bool,
}
#[derive(Deserialize)]
pub struct ThreadListQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub journal_id: Option<Uuid>,
    #[serde(default)]
    pub q: String,
}
