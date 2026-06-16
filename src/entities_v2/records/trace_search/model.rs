use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Float, Text, Timestamp, Uuid as SqlUuid};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    trace::{Trace, TraceStatus},
};
use crate::pagination::PaginationParams;

#[derive(Debug)]
pub struct TraceSearchDocument;

#[derive(Debug, Deserialize)]
pub struct TraceSearchParams {
    pub q: String,
    pub journal_id: Option<Uuid>,
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

#[derive(Debug, Serialize)]
pub struct TraceSearchItem {
    pub trace: Trace,
    pub score: f32,
    pub matched_sources: Vec<String>,
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct TraceSearchRow {
    #[diesel(sql_type = SqlUuid)]
    trace_id: Uuid,
    #[diesel(sql_type = Float)]
    score: f32,
    #[diesel(sql_type = Text)]
    matched_sources: String,
}

impl TraceSearchDocument {
    pub fn refresh_for_trace(trace_id: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
        let trace = Trace::find_full_trace(trace_id, pool)?;
        let mut conn = pool.get()?;

        if trace.status != TraceStatus::Finalized {
            diesel::sql_query("DELETE FROM trace_search_documents WHERE trace_id = $1")
                .bind::<SqlUuid, _>(trace_id)
                .execute(&mut conn)?;
            return Ok(());
        }

        let Some(journal_id) = trace.journal_id else {
            diesel::sql_query("DELETE FROM trace_search_documents WHERE trace_id = $1")
                .bind::<SqlUuid, _>(trace_id)
                .execute(&mut conn)?;
            return Ok(());
        };

        sql_query(
            r#"
            WITH mirror_parts AS (
                SELECT
                    COALESCE(string_agg(CONCAT_WS(' ', tm.title, tm.subtitle), ' '), '') AS mirror_text,
                    COALESCE(string_agg(tag_value, ' '), '') AS tag_text
                FROM trace_mirrors tm
                INNER JOIN users u
                  ON u.id = $2
                INNER JOIN lens_analysis_scopes las
                  ON las.landscape_analysis_id = tm.landscape_analysis_id
                 AND las.lens_id = u.current_lens_id
                LEFT JOIN LATERAL jsonb_array_elements_text(tm.tags) AS tag_value ON TRUE
                WHERE tm.trace_id = $1
            ),
            element_parts AS (
                SELECT COALESCE(string_agg(CONCAT_WS(' ', e.title, e.subtitle, e.verb), ' '), '') AS element_text
                FROM elements e
                INNER JOIN users u
                  ON u.id = $2
                INNER JOIN lens_analysis_scopes las
                  ON las.landscape_analysis_id = e.analysis_id
                 AND las.lens_id = u.current_lens_id
                WHERE e.trace_id = $1
            ),
            landmark_parts AS (
                SELECT COALESCE(string_agg(DISTINCT CONCAT_WS(' ', l.title, l.subtitle), ' '), '') AS landmark_text
                FROM elements e
                INNER JOIN users u
                  ON u.id = $2
                INNER JOIN lens_analysis_scopes las
                  ON las.landscape_analysis_id = e.analysis_id
                 AND las.lens_id = u.current_lens_id
                JOIN element_landmarks el
                  ON el.element_id = e.id
                JOIN landmarks l
                  ON l.id = el.landmark_id
                WHERE e.trace_id = $1
            ),
            primary_landmark_parts AS (
                SELECT COALESCE(string_agg(DISTINCT CONCAT_WS(' ', l.title, l.subtitle), ' '), '') AS landmark_text
                FROM trace_mirrors tm
                INNER JOIN users u
                  ON u.id = $2
                INNER JOIN lens_analysis_scopes las
                  ON las.landscape_analysis_id = tm.landscape_analysis_id
                 AND las.lens_id = u.current_lens_id
                JOIN landmarks l
                  ON l.id = tm.primary_landmark_id
                WHERE tm.trace_id = $1
            ),
            direct_landmark_parts AS (
                SELECT COALESCE(string_agg(DISTINCT CONCAT_WS(' ', l.title, l.subtitle), ' '), '') AS landmark_text
                FROM landscape_analyses la
                INNER JOIN users u
                  ON u.id = $2
                INNER JOIN lens_analysis_scopes las
                  ON las.landscape_analysis_id = la.id
                 AND las.lens_id = u.current_lens_id
                INNER JOIN landscape_landmarks ll
                  ON ll.landscape_analysis_id = la.id
                INNER JOIN landmarks l
                  ON l.id = ll.landmark_id
                WHERE la.analyzed_trace_id = $1
            ),
            doc AS (
                SELECT
                    $1::uuid AS trace_id,
                    $2::uuid AS user_id,
                    $3::uuid AS journal_id,
                    $4::timestamp AS interaction_date,
                    $5::text AS title,
                    $6::text AS content,
                    mp.mirror_text,
                    mp.tag_text,
                    ep.element_text,
                    CONCAT_WS(' ', lp.landmark_text, plp.landmark_text, dlp.landmark_text) AS landmark_text
                FROM mirror_parts mp
                CROSS JOIN element_parts ep
                CROSS JOIN landmark_parts lp
                CROSS JOIN primary_landmark_parts plp
                CROSS JOIN direct_landmark_parts dlp
            )
            INSERT INTO trace_search_documents (
                trace_id,
                user_id,
                journal_id,
                interaction_date,
                title,
                content,
                mirror_text,
                tag_text,
                element_text,
                landmark_text,
                search_vector,
                refreshed_at
            )
            SELECT
                trace_id,
                user_id,
                journal_id,
                interaction_date,
                title,
                content,
                mirror_text,
                tag_text,
                element_text,
                landmark_text,
                setweight(to_tsvector('simple', title), 'A') ||
                setweight(to_tsvector('simple', content), 'A') ||
                setweight(to_tsvector('simple', tag_text), 'A') ||
                setweight(to_tsvector('simple', mirror_text), 'B') ||
                setweight(to_tsvector('simple', element_text), 'C') ||
                setweight(to_tsvector('simple', landmark_text), 'C'),
                NOW()
            FROM doc
            ON CONFLICT (trace_id) DO UPDATE
            SET user_id = EXCLUDED.user_id,
                journal_id = EXCLUDED.journal_id,
                interaction_date = EXCLUDED.interaction_date,
                title = EXCLUDED.title,
                content = EXCLUDED.content,
                mirror_text = EXCLUDED.mirror_text,
                tag_text = EXCLUDED.tag_text,
                element_text = EXCLUDED.element_text,
                landmark_text = EXCLUDED.landmark_text,
                search_vector = EXCLUDED.search_vector,
                refreshed_at = NOW()
            "#,
        )
        .bind::<SqlUuid, _>(trace.id)
        .bind::<SqlUuid, _>(trace.user_id)
        .bind::<SqlUuid, _>(journal_id)
        .bind::<Timestamp, _>(trace.interaction_date)
        .bind::<Text, _>(trace.title)
        .bind::<Text, _>(trace.content)
        .execute(&mut conn)?;

        Ok(())
    }

    pub fn search_for_user(
        user_id: Uuid,
        query: &str,
        journal_id: Option<Uuid>,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<TraceSearchItem>, i64), PpdcError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "q is required".to_string(),
            ));
        }

        let mut conn = pool.get()?;
        let total = sql_query(
            r#"
            WITH q AS (SELECT websearch_to_tsquery('simple', $2) AS query)
            SELECT COUNT(*)::bigint AS count
            FROM trace_search_documents tsd, q
            WHERE tsd.user_id = $1
              AND ($3::uuid IS NULL OR tsd.journal_id = $3)
              AND tsd.search_vector @@ q.query
            "#,
        )
        .bind::<SqlUuid, _>(user_id)
        .bind::<Text, _>(query)
        .bind::<diesel::sql_types::Nullable<SqlUuid>, _>(journal_id)
        .get_result::<CountRow>(&mut conn)?
        .count;

        let rows = sql_query(
            r#"
            WITH q AS (SELECT websearch_to_tsquery('simple', $2) AS query)
            SELECT
                tsd.trace_id,
                ts_rank_cd(tsd.search_vector, q.query)::real AS score,
                ARRAY_TO_STRING(
                    ARRAY_REMOVE(ARRAY[
                        CASE WHEN setweight(to_tsvector('simple', tsd.title), 'A') @@ q.query
                               OR setweight(to_tsvector('simple', tsd.content), 'A') @@ q.query
                             THEN 'trace' END,
                        CASE WHEN setweight(to_tsvector('simple', tsd.tag_text), 'A') @@ q.query
                             THEN 'tag' END,
                        CASE WHEN setweight(to_tsvector('simple', tsd.mirror_text), 'B') @@ q.query
                             THEN 'trace_mirror' END,
                        CASE WHEN setweight(to_tsvector('simple', tsd.element_text), 'C') @@ q.query
                             THEN 'element' END,
                        CASE WHEN setweight(to_tsvector('simple', tsd.landmark_text), 'C') @@ q.query
                             THEN 'landmark' END
                    ], NULL),
                    ','
                ) AS matched_sources
            FROM trace_search_documents tsd, q
            WHERE tsd.user_id = $1
              AND ($3::uuid IS NULL OR tsd.journal_id = $3)
              AND tsd.search_vector @@ q.query
            ORDER BY score DESC, tsd.interaction_date DESC
            OFFSET $4
            LIMIT $5
            "#,
        )
        .bind::<SqlUuid, _>(user_id)
        .bind::<Text, _>(query)
        .bind::<diesel::sql_types::Nullable<SqlUuid>, _>(journal_id)
        .bind::<BigInt, _>(offset)
        .bind::<BigInt, _>(limit)
        .load::<TraceSearchRow>(&mut conn)?;

        let items = rows
            .into_iter()
            .map(|row| {
                let trace = Trace::find_full_trace(row.trace_id, pool)?;
                Ok(TraceSearchItem {
                    trace,
                    score: row.score,
                    matched_sources: row
                        .matched_sources
                        .split(',')
                        .filter(|value| !value.is_empty())
                        .map(|value| value.to_string())
                        .collect(),
                })
            })
            .collect::<Result<Vec<_>, PpdcError>>()?;

        Ok((items, total))
    }
}
