use chrono::{Duration, NaiveDate, Utc};
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Date, Uuid as SqlUuid};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use axum::{
    debug_handler,
    extract::{Extension, Path, Query},
    Json,
};
use crate::db::DbPool;
use crate::entities::error::PpdcError;

#[derive(Debug, QueryableByName, Serialize)]
pub struct HeatmapRow {
    #[diesel(sql_type = Date)]
    pub day: NaiveDate,
    #[diesel(sql_type = BigInt)]
    pub value: i64,
}


pub fn heatmap_sum_trace_content_len_join_direct(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
    user_id: Uuid,
) -> QueryResult<Vec<HeatmapRow>> {
    let sql = r#"
WITH days AS (
  SELECT generate_series($1::date, $2::date, interval '1 day')::date AS day
),
agg AS (
  SELECT
    i.interaction_date::date AS day,
    SUM(length(r.content))::bigint AS value
  FROM resources r
  JOIN interactions i
    ON i.resource_id = r.id
  WHERE i.interaction_user_id = $3
    AND r.entity_type = 'trce'
    AND i.interaction_date >= $1::date
    AND i.interaction_date <  ($2::date + 1)
  GROUP BY 1
)
SELECT d.day AS day, COALESCE(a.value, 0) AS value
FROM days d
LEFT JOIN agg a USING (day)
ORDER BY d.day;
"#;

    diesel::sql_query(sql)
        .bind::<Date, _>(from)
        .bind::<Date, _>(to)
        .bind::<SqlUuid, _>(user_id)
        .load::<HeatmapRow>(conn)
}

#[derive(Debug, Deserialize)]
pub struct HeatmapParams {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
}

#[debug_handler]
pub async fn get_user_heatmap_route(
    Extension(pool): Extension<DbPool>,
    Path(user_id): Path<Uuid>,
    Query(params): Query<HeatmapParams>,
) -> Result<Json<Vec<HeatmapRow>>, PpdcError> {
    let to = params.to.unwrap_or_else(|| Utc::now().date_naive());
    let from = params.from.unwrap_or_else(|| to - Duration::days(270));
    if from > to {
        return Err(PpdcError::new(
            400,
            crate::entities::error::ErrorType::ApiError,
            "`from` must be <= `to`".to_string(),
        ));
    }
    let mut conn = pool.get().expect("Failed to get a connection from the pool");
    let rows = heatmap_sum_trace_content_len_join_direct(
        &mut conn,
        from,
        to,
        user_id,
    )?;
    Ok(Json(rows))
}
