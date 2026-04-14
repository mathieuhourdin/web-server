use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::entities_v2::{landscape_analysis::LandscapeAnalysis, session::Session};
use crate::pagination::{PaginatedResponse, PaginationParams};
use crate::schema::{landscape_analyses, llm_calls};
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
#[diesel(check_for_backend(diesel::pg::Pg))]
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

#[derive(Debug, Deserialize)]
pub struct LlmCallFiltersQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub created_at_from: Option<NaiveDateTime>,
    pub created_at_to: Option<NaiveDateTime>,
}

impl LlmCall {
    pub fn get_paginated_for_user(
        user_id: Uuid,
        offset: i64,
        limit: i64,
        db: &DbPool,
    ) -> Result<Vec<Self>, PpdcError> {
        let (items, _) =
            Self::get_paginated_for_user_filtered(user_id, offset, limit, None, None, db)?;
        Ok(items)
    }

    pub fn get_paginated_for_user_filtered(
        user_id: Uuid,
        offset: i64,
        limit: i64,
        created_at_from: Option<NaiveDateTime>,
        created_at_to: Option<NaiveDateTime>,
        db: &DbPool,
    ) -> Result<(Vec<Self>, i64), PpdcError> {
        let mut conn = db.get()?;
        let mut count_query = llm_calls::table
            .inner_join(
                landscape_analyses::table
                    .on(llm_calls::analysis_id.eq(landscape_analyses::id.nullable())),
            )
            .filter(landscape_analyses::user_id.eq(user_id))
            .into_boxed();
        if let Some(from) = created_at_from {
            count_query = count_query.filter(llm_calls::created_at.ge(from));
        }
        if let Some(to) = created_at_to {
            count_query = count_query.filter(llm_calls::created_at.le(to));
        }
        let total = count_query.count().get_result::<i64>(&mut conn)?;
        let mut query = llm_calls::table
            .inner_join(
                landscape_analyses::table
                    .on(llm_calls::analysis_id.eq(landscape_analyses::id.nullable())),
            )
            .filter(landscape_analyses::user_id.eq(user_id))
            .into_boxed();
        if let Some(from) = created_at_from {
            query = query.filter(llm_calls::created_at.ge(from));
        }
        if let Some(to) = created_at_to {
            query = query.filter(llm_calls::created_at.le(to));
        }
        let llm_calls = query
            .select(LlmCall::as_select())
            .order(llm_calls::created_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<Self>(&mut conn)?;
        Ok((llm_calls, total))
    }

    pub fn get_by_id_for_user(id: Uuid, user_id: Uuid, db: &DbPool) -> Result<Self, PpdcError> {
        let mut conn = db.get()?;
        let llm_call = llm_calls::table
            .inner_join(
                landscape_analyses::table
                    .on(llm_calls::analysis_id.eq(landscape_analyses::id.nullable())),
            )
            .filter(llm_calls::id.eq(id))
            .filter(landscape_analyses::user_id.eq(user_id))
            .select(LlmCall::as_select())
            .first::<Self>(&mut conn)?;
        Ok(llm_call)
    }

    pub fn get_paginated(offset: i64, limit: i64, db: &DbPool) -> Result<Vec<Self>, PpdcError> {
        let mut conn = db.get()?;
        let llm_calls = llm_calls::table
            .select(LlmCall::as_select())
            .order(llm_calls::created_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<Self>(&mut conn)?;
        Ok(llm_calls)
    }
    pub fn get_by_id(id: Uuid, db: &DbPool) -> Result<Self, PpdcError> {
        let mut conn = db.get()?;
        let llm_call = llm_calls::table
            .select(LlmCall::as_select())
            .filter(llm_calls::id.eq(id))
            .first::<Self>(&mut conn)?;
        Ok(llm_call)
    }
    pub fn get_by_analysis_id(analysis_id: Uuid, db: &DbPool) -> Result<Vec<Self>, PpdcError> {
        let mut conn = db.get()?;
        let llm_calls = llm_calls::table
            .select(LlmCall::as_select())
            .filter(llm_calls::analysis_id.eq(analysis_id))
            .load::<Self>(&mut conn)?;
        Ok(llm_calls)
    }

    pub fn get_by_analysis_id_for_user(
        analysis_id: Uuid,
        user_id: Uuid,
        db: &DbPool,
    ) -> Result<Vec<Self>, PpdcError> {
        let (items, _) = Self::get_by_analysis_id_for_user_filtered(
            analysis_id,
            user_id,
            0,
            i64::MAX / 4,
            None,
            None,
            db,
        )?;
        Ok(items)
    }

    pub fn get_by_analysis_id_for_user_filtered(
        analysis_id: Uuid,
        user_id: Uuid,
        offset: i64,
        limit: i64,
        created_at_from: Option<NaiveDateTime>,
        created_at_to: Option<NaiveDateTime>,
        db: &DbPool,
    ) -> Result<(Vec<Self>, i64), PpdcError> {
        let mut conn = db.get()?;
        let mut count_query = llm_calls::table
            .inner_join(
                landscape_analyses::table
                    .on(llm_calls::analysis_id.eq(landscape_analyses::id.nullable())),
            )
            .filter(llm_calls::analysis_id.eq(analysis_id))
            .filter(landscape_analyses::user_id.eq(user_id))
            .into_boxed();
        if let Some(from) = created_at_from {
            count_query = count_query.filter(llm_calls::created_at.ge(from));
        }
        if let Some(to) = created_at_to {
            count_query = count_query.filter(llm_calls::created_at.le(to));
        }
        let total = count_query.count().get_result::<i64>(&mut conn)?;
        let mut query = llm_calls::table
            .inner_join(
                landscape_analyses::table
                    .on(llm_calls::analysis_id.eq(landscape_analyses::id.nullable())),
            )
            .filter(llm_calls::analysis_id.eq(analysis_id))
            .filter(landscape_analyses::user_id.eq(user_id))
            .into_boxed();
        if let Some(from) = created_at_from {
            query = query.filter(llm_calls::created_at.ge(from));
        }
        if let Some(to) = created_at_to {
            query = query.filter(llm_calls::created_at.le(to));
        }
        let llm_calls = query
            .select(LlmCall::as_select())
            .order(llm_calls::created_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<Self>(&mut conn)?;
        Ok((llm_calls, total))
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
        let mut conn = db.get()?;
        let llm_call = diesel::insert_into(llm_calls::table)
            .values(&self)
            .returning(LlmCall::as_returning())
            .get_result(&mut conn)?;
        Ok(llm_call)
    }
}

#[debug_handler]
pub async fn get_llm_calls_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(filters): Query<LlmCallFiltersQuery>,
) -> Result<Json<PaginatedResponse<LlmCall>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = filters.pagination.validate()?;
    let (llm_calls, total) = LlmCall::get_paginated_for_user_filtered(
        user_id,
        pagination.offset,
        pagination.limit,
        filters.created_at_from,
        filters.created_at_to,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(llm_calls, pagination, total)))
}

#[debug_handler]
pub async fn get_llm_calls_by_analysis_id_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(analysis_id): Path<Uuid>,
    Query(filters): Query<LlmCallFiltersQuery>,
) -> Result<Json<PaginatedResponse<LlmCall>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let analysis = LandscapeAnalysis::find_full_analysis(analysis_id, &pool)?;
    if analysis.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let pagination = filters.pagination.validate()?;
    let (llm_calls, total) = LlmCall::get_by_analysis_id_for_user_filtered(
        analysis_id,
        user_id,
        pagination.offset,
        pagination.limit,
        filters.created_at_from,
        filters.created_at_to,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(llm_calls, pagination, total)))
}
#[debug_handler]
pub async fn get_llm_call_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<LlmCall>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let llm_call = LlmCall::get_by_id_for_user(id, user_id, &pool)?;
    Ok(Json(llm_call))
}
