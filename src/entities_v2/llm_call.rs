use crate::db::DbPool;
use crate::entities::error::PpdcError;
use crate::pagination::PaginationParams;
use crate::schema::llm_calls;
use axum::{
    debug_handler,
    extract::{Extension, Path, Query},
    Json,
};
use chrono::NaiveDateTime;
use diesel;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, Insertable, AsChangeset, Clone)]
#[diesel(table_name = crate::schema::llm_calls)]
pub struct LlmCall {
    pub id: Uuid,
    pub status: String,
    pub model: String,
    pub prompt: String,
    pub display_name: String,
    pub schema: String,
    pub request: String,
    pub request_url: String,
    pub response: String,
    pub output: String,
    pub input_tokens_used: i32,
    pub reasoning_tokens_used: i32,
    pub output_tokens_used: i32,
    pub price: f64,
    pub currency: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub analysis_id: Option<Uuid>,
    pub system_prompt: String,
    pub user_prompt: String,
}

#[derive(Insertable, AsChangeset)]
#[diesel(table_name = crate::schema::llm_calls)]
pub struct NewLlmCall {
    pub status: String,
    pub model: String,
    pub prompt: String,
    pub display_name: String,
    pub schema: String,
    pub request: String,
    pub request_url: String,
    pub response: String,
    pub output: String,
    pub input_tokens_used: i32,
    pub reasoning_tokens_used: i32,
    pub output_tokens_used: i32,
    pub price: f64,
    pub currency: String,
    pub analysis_id: Uuid,
    pub system_prompt: String,
    pub user_prompt: String,
}

impl LlmCall {
    pub fn get_paginated(offset: i64, limit: i64, db: &DbPool) -> Result<Vec<Self>, PpdcError> {
        let mut conn = db.get().expect("Failed to get a connection from the pool");
        let llm_calls = llm_calls::table
            .order(llm_calls::created_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<Self>(&mut conn)?;
        Ok(llm_calls)
    }
    pub fn get_by_id(id: Uuid, db: &DbPool) -> Result<Self, PpdcError> {
        let mut conn = db.get().expect("Failed to get a connection from the pool");
        let llm_call = llm_calls::table
            .filter(llm_calls::id.eq(id))
            .first(&mut conn)?;
        Ok(llm_call)
    }
    pub fn get_by_analysis_id(analysis_id: Uuid, db: &DbPool) -> Result<Vec<Self>, PpdcError> {
        let mut conn = db.get().expect("Failed to get a connection from the pool");
        let llm_calls = llm_calls::table
            .filter(llm_calls::analysis_id.eq(analysis_id))
            .load::<Self>(&mut conn)?;
        Ok(llm_calls)
    }
}

impl NewLlmCall {
    pub fn new(
        status: String,
        model: String,
        prompt: String,
        display_name: String,
        schema: String,
        request: String,
        request_url: String,
        response: String,
        output: String,
        input_tokens_used: i32,
        reasoning_tokens_used: i32,
        output_tokens_used: i32,
        price: f64,
        currency: String,
        analysis_id: Uuid,
        system_prompt: String,
        user_prompt: String,
    ) -> Self {
        Self {
            status,
            model,
            prompt,
            display_name,
            schema,
            request,
            request_url,
            response,
            output,
            input_tokens_used,
            reasoning_tokens_used,
            output_tokens_used,
            price,
            currency,
            analysis_id,
            system_prompt,
            user_prompt,
        }
    }

    pub fn create(self, db: &DbPool) -> Result<LlmCall, PpdcError> {
        let mut conn = db.get().expect("Failed to get a connection from the pool");
        let llm_call = diesel::insert_into(llm_calls::table)
            .values(&self)
            .get_result(&mut conn)?;
        Ok(llm_call)
    }
}

#[debug_handler]
pub async fn get_llm_calls_route(
    Extension(pool): Extension<DbPool>,
    Query(pagination): Query<PaginationParams>,
) -> Result<Json<Vec<LlmCall>>, PpdcError> {
    let llm_calls = LlmCall::get_paginated(pagination.offset(), pagination.limit(), &pool)?;
    Ok(Json(llm_calls))
}

#[debug_handler]
pub async fn get_llm_calls_by_analysis_id_route(
    Extension(pool): Extension<DbPool>,
    Path(analysis_id): Path<Uuid>,
) -> Result<Json<Vec<LlmCall>>, PpdcError> {
    let llm_calls = LlmCall::get_by_analysis_id(analysis_id, &pool)?;
    Ok(Json(llm_calls))
}
#[debug_handler]
pub async fn get_llm_call_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<LlmCall>, PpdcError> {
    let llm_call = LlmCall::get_by_id(id, &pool)?;
    Ok(Json(llm_call))
}
