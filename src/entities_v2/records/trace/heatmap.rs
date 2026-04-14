use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::session::Session;
use crate::schema::traces;
use axum::{
    debug_handler,
    extract::{Extension, Path, Query},
    Json,
};
use chrono::{Duration, NaiveDate, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct HeatmapRow {
    pub day: NaiveDate,
    pub value: i64,
}

type TraceContentRow = (chrono::NaiveDateTime, String);

pub fn heatmap_sum_trace_content_len(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
    user_id: Uuid,
) -> QueryResult<Vec<HeatmapRow>> {
    let from_dt = from.and_hms_opt(0, 0, 0).expect("valid day start");
    let to_exclusive_dt = (to + Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .expect("valid day start");

    let rows = traces::table
        .filter(traces::user_id.eq(user_id))
        .filter(traces::interaction_date.ge(from_dt))
        .filter(traces::interaction_date.lt(to_exclusive_dt))
        .select((traces::interaction_date, traces::content))
        .load::<TraceContentRow>(conn)?;

    let mut by_day = HashMap::<NaiveDate, i64>::new();
    for (interaction_date, content) in rows {
        let day = interaction_date.date();
        let value = i64::try_from(content.chars().count()).unwrap_or(i64::MAX);
        *by_day.entry(day).or_insert(0) += value;
    }

    let mut out = Vec::new();
    let mut day = from;
    while day <= to {
        out.push(HeatmapRow {
            day,
            value: by_day.get(&day).copied().unwrap_or(0),
        });
        day += Duration::days(1);
    }

    Ok(out)
}

#[derive(Debug, Deserialize)]
pub struct HeatmapParams {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
}

#[debug_handler]
pub async fn get_user_heatmap_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(user_id): Path<Uuid>,
    Query(params): Query<HeatmapParams>,
) -> Result<Json<Vec<HeatmapRow>>, PpdcError> {
    let session_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    if session_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let to = params.to.unwrap_or_else(|| Utc::now().date_naive());
    let from = params.from.unwrap_or_else(|| to - Duration::days(270));
    if from > to {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "`from` must be <= `to`".to_string(),
        ));
    }
    let mut conn = pool
        .get()?;
    let rows = heatmap_sum_trace_content_len(&mut conn, from, to, user_id)?;
    Ok(Json(rows))
}
