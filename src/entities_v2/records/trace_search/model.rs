use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Array, BigInt, Bool, Float, Int4, Text, Timestamp, Uuid as SqlUuid};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

#[derive(Debug, Serialize)]
pub struct TraceSearchItem {
    pub trace: Trace,
    pub score: f32,
    pub matched_sources: Vec<String>,
    pub landmarks: Vec<TraceSearchLandmark>,
    pub high_level_project_landmarks: Vec<TraceSearchLandmark>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceSearchLandmark {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub score: f32,
    pub matched_query: bool,
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

#[derive(QueryableByName)]
struct TraceSearchLandmarkRow {
    #[diesel(sql_type = SqlUuid)]
    trace_id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    landmark_id: Uuid,
    #[diesel(sql_type = Text)]
    title: String,
    #[diesel(sql_type = Text)]
    subtitle: String,
    #[diesel(sql_type = Float)]
    score: f32,
    #[diesel(sql_type = Bool)]
    matched_query: bool,
    #[diesel(sql_type = Bool)]
    is_high_level_project: bool,
    #[diesel(sql_type = Int4)]
    #[allow(dead_code)]
    row_rank: i32,
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
                INNER JOIN landscape_analyses la
                  ON la.id = tm.landscape_analysis_id
                 AND la.landscape_analysis_type = 'TRACE_INCREMENTAL'
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
                INNER JOIN landscape_analyses la
                  ON la.id = e.analysis_id
                 AND la.landscape_analysis_type = 'TRACE_INCREMENTAL'
                INNER JOIN users u
                  ON u.id = $2
                INNER JOIN lens_analysis_scopes las
                  ON las.landscape_analysis_id = e.analysis_id
                 AND las.lens_id = u.current_lens_id
                WHERE e.trace_id = $1
            ),
            reference_parts AS (
                SELECT COALESCE(string_agg(CONCAT_WS(' ', r.mention, tags.context_tags, variants.reference_variants), ' '), '') AS reference_text
                FROM "references" r
                INNER JOIN trace_mirrors tm
                  ON tm.id = r.trace_mirror_id
                INNER JOIN landscape_analyses la
                  ON la.id = r.landscape_analysis_id
                 AND la.landscape_analysis_type = 'TRACE_INCREMENTAL'
                INNER JOIN users u
                  ON u.id = $2
                INNER JOIN lens_analysis_scopes las
                  ON las.landscape_analysis_id = r.landscape_analysis_id
                 AND las.lens_id = u.current_lens_id
                LEFT JOIN landmarks l
                  ON l.id = r.landmark_id
                LEFT JOIN LATERAL (
                    SELECT COALESCE(string_agg(value, ' '), '') AS context_tags
                    FROM jsonb_array_elements_text(r.context_tags) AS tag_value(value)
                ) tags ON TRUE
                LEFT JOIN LATERAL (
                    SELECT COALESCE(string_agg(value, ' '), '') AS reference_variants
                    FROM jsonb_array_elements_text(r.reference_variants) AS variant_value(value)
                ) variants ON TRUE
                WHERE tm.trace_id = $1
                  AND (l.id IS NULL OR l.landmark_type <> 'HIGH_LEVEL_PROJECT')
            ),
            reference_landmark_parts AS (
                SELECT COALESCE(string_agg(DISTINCT CONCAT_WS(' ', l.title, l.subtitle), ' '), '') AS landmark_text
                FROM "references" r
                INNER JOIN trace_mirrors tm
                  ON tm.id = r.trace_mirror_id
                INNER JOIN landscape_analyses la
                  ON la.id = r.landscape_analysis_id
                 AND la.landscape_analysis_type = 'TRACE_INCREMENTAL'
                INNER JOIN users u
                  ON u.id = $2
                INNER JOIN lens_analysis_scopes las
                  ON las.landscape_analysis_id = r.landscape_analysis_id
                 AND las.lens_id = u.current_lens_id
                JOIN landmarks l
                  ON l.id = r.landmark_id
                WHERE tm.trace_id = $1
                  AND r.landmark_id IS NOT NULL
                  AND l.landmark_type <> 'HIGH_LEVEL_PROJECT'
            ),
            high_level_project_landmark_parts AS (
                SELECT COALESCE(string_agg(DISTINCT CONCAT_WS(' ', l.title, l.subtitle), ' '), '') AS high_level_project_landmark_text
                FROM "references" r
                INNER JOIN trace_mirrors tm
                  ON tm.id = r.trace_mirror_id
                INNER JOIN landscape_analyses la
                  ON la.id = r.landscape_analysis_id
                 AND la.landscape_analysis_type = 'TRACE_INCREMENTAL'
                INNER JOIN users u
                  ON u.id = $2
                INNER JOIN lens_analysis_scopes las
                  ON las.landscape_analysis_id = r.landscape_analysis_id
                 AND las.lens_id = u.current_lens_id
                JOIN landmarks l
                  ON l.id = r.landmark_id
                WHERE tm.trace_id = $1
                  AND r.landmark_id IS NOT NULL
                  AND l.landmark_type = 'HIGH_LEVEL_PROJECT'
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
                    rp.reference_text,
                    rlp.landmark_text,
                    hlp.high_level_project_landmark_text
                FROM mirror_parts mp
                CROSS JOIN element_parts ep
                CROSS JOIN reference_parts rp
                CROSS JOIN reference_landmark_parts rlp
                CROSS JOIN high_level_project_landmark_parts hlp
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
                reference_text,
                landmark_text,
                high_level_project_landmark_text,
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
                reference_text,
                landmark_text,
                high_level_project_landmark_text,
                setweight(to_tsvector('simple', title), 'A') ||
                setweight(to_tsvector('simple', content), 'A') ||
                setweight(to_tsvector('simple', tag_text), 'A') ||
                setweight(to_tsvector('simple', mirror_text), 'B') ||
                setweight(to_tsvector('simple', element_text), 'C') ||
                setweight(to_tsvector('simple', reference_text), 'C') ||
                setweight(to_tsvector('simple', landmark_text), 'C') ||
                setweight(to_tsvector('simple', high_level_project_landmark_text), 'C'),
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
                reference_text = EXCLUDED.reference_text,
                landmark_text = EXCLUDED.landmark_text,
                high_level_project_landmark_text = EXCLUDED.high_level_project_landmark_text,
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
        journal_ids: &[Uuid],
        landmark_ids: &[Uuid],
        high_level_project_landmark_ids: &[Uuid],
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
              AND (cardinality($3::uuid[]) = 0 OR tsd.journal_id = ANY($3))
              AND tsd.search_vector @@ q.query
              AND (
                cardinality($4::uuid[]) = 0
                OR EXISTS (
                    SELECT 1
                    FROM "references" r
                    INNER JOIN trace_mirrors tm
                      ON tm.id = r.trace_mirror_id
                    INNER JOIN landscape_analyses la
                      ON la.id = r.landscape_analysis_id
                     AND la.landscape_analysis_type = 'TRACE_INCREMENTAL'
                    INNER JOIN users u
                      ON u.id = $1
                    INNER JOIN lens_analysis_scopes las
                      ON las.landscape_analysis_id = r.landscape_analysis_id
                     AND las.lens_id = u.current_lens_id
                    INNER JOIN landmarks l
                      ON l.id = r.landmark_id
                    WHERE tm.trace_id = tsd.trace_id
                      AND r.landmark_id = ANY($4)
                      AND l.landmark_type <> 'HIGH_LEVEL_PROJECT'
                )
              )
              AND (
                cardinality($5::uuid[]) = 0
                OR EXISTS (
                    SELECT 1
                    FROM "references" r
                    INNER JOIN trace_mirrors tm
                      ON tm.id = r.trace_mirror_id
                    INNER JOIN landscape_analyses la
                      ON la.id = r.landscape_analysis_id
                     AND la.landscape_analysis_type = 'TRACE_INCREMENTAL'
                    INNER JOIN users u
                      ON u.id = $1
                    INNER JOIN lens_analysis_scopes las
                      ON las.landscape_analysis_id = r.landscape_analysis_id
                     AND las.lens_id = u.current_lens_id
                    INNER JOIN landmarks l
                      ON l.id = r.landmark_id
                    WHERE tm.trace_id = tsd.trace_id
                      AND r.landmark_id = ANY($5)
                      AND l.landmark_type = 'HIGH_LEVEL_PROJECT'
                )
              )
            "#,
        )
        .bind::<SqlUuid, _>(user_id)
        .bind::<Text, _>(query)
        .bind::<Array<SqlUuid>, _>(journal_ids)
        .bind::<Array<SqlUuid>, _>(landmark_ids)
        .bind::<Array<SqlUuid>, _>(high_level_project_landmark_ids)
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
                        CASE WHEN setweight(to_tsvector('simple', tsd.reference_text), 'C') @@ q.query
                             THEN 'reference' END,
                        CASE WHEN setweight(to_tsvector('simple', tsd.landmark_text), 'C') @@ q.query
                             THEN 'landmark' END,
                        CASE WHEN setweight(to_tsvector('simple', tsd.high_level_project_landmark_text), 'C') @@ q.query
                             THEN 'high_level_project_landmark' END
                    ], NULL),
                    ','
                ) AS matched_sources
            FROM trace_search_documents tsd, q
            WHERE tsd.user_id = $1
              AND (cardinality($3::uuid[]) = 0 OR tsd.journal_id = ANY($3))
              AND tsd.search_vector @@ q.query
              AND (
                cardinality($4::uuid[]) = 0
                OR EXISTS (
                    SELECT 1
                    FROM "references" r
                    INNER JOIN trace_mirrors tm
                      ON tm.id = r.trace_mirror_id
                    INNER JOIN landscape_analyses la
                      ON la.id = r.landscape_analysis_id
                     AND la.landscape_analysis_type = 'TRACE_INCREMENTAL'
                    INNER JOIN users u
                      ON u.id = $1
                    INNER JOIN lens_analysis_scopes las
                      ON las.landscape_analysis_id = r.landscape_analysis_id
                     AND las.lens_id = u.current_lens_id
                    INNER JOIN landmarks l
                      ON l.id = r.landmark_id
                    WHERE tm.trace_id = tsd.trace_id
                      AND r.landmark_id = ANY($4)
                      AND l.landmark_type <> 'HIGH_LEVEL_PROJECT'
                )
              )
              AND (
                cardinality($5::uuid[]) = 0
                OR EXISTS (
                    SELECT 1
                    FROM "references" r
                    INNER JOIN trace_mirrors tm
                      ON tm.id = r.trace_mirror_id
                    INNER JOIN landscape_analyses la
                      ON la.id = r.landscape_analysis_id
                     AND la.landscape_analysis_type = 'TRACE_INCREMENTAL'
                    INNER JOIN users u
                      ON u.id = $1
                    INNER JOIN lens_analysis_scopes las
                      ON las.landscape_analysis_id = r.landscape_analysis_id
                     AND las.lens_id = u.current_lens_id
                    INNER JOIN landmarks l
                      ON l.id = r.landmark_id
                    WHERE tm.trace_id = tsd.trace_id
                      AND r.landmark_id = ANY($5)
                      AND l.landmark_type = 'HIGH_LEVEL_PROJECT'
                )
              )
            ORDER BY score DESC, tsd.interaction_date DESC
            OFFSET $6
            LIMIT $7
            "#,
        )
        .bind::<SqlUuid, _>(user_id)
        .bind::<Text, _>(query)
        .bind::<Array<SqlUuid>, _>(journal_ids)
        .bind::<Array<SqlUuid>, _>(landmark_ids)
        .bind::<Array<SqlUuid>, _>(high_level_project_landmark_ids)
        .bind::<BigInt, _>(offset)
        .bind::<BigInt, _>(limit)
        .load::<TraceSearchRow>(&mut conn)?;

        let trace_ids = rows.iter().map(|row| row.trace_id).collect::<Vec<_>>();
        let landmark_context =
            Self::hydrate_ranked_landmarks_for_traces(user_id, query, &trace_ids, pool)?;

        let items = rows
            .into_iter()
            .map(|row| {
                let trace = Trace::find_full_trace(row.trace_id, pool)?;
                let context = landmark_context.get(&row.trace_id);
                Ok(TraceSearchItem {
                    trace,
                    score: row.score,
                    matched_sources: row
                        .matched_sources
                        .split(',')
                        .filter(|value| !value.is_empty())
                        .map(|value| value.to_string())
                        .collect(),
                    landmarks: context
                        .map(|context| context.landmarks.clone())
                        .unwrap_or_default(),
                    high_level_project_landmarks: context
                        .map(|context| context.high_level_project_landmarks.clone())
                        .unwrap_or_default(),
                })
            })
            .collect::<Result<Vec<_>, PpdcError>>()?;

        Ok((items, total))
    }

    fn hydrate_ranked_landmarks_for_traces(
        user_id: Uuid,
        query: &str,
        trace_ids: &[Uuid],
        pool: &DbPool,
    ) -> Result<HashMap<Uuid, TraceSearchLandmarkContext>, PpdcError> {
        if trace_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut conn = pool.get()?;
        let rows = sql_query(
            r#"
            WITH input_trace_ids AS (
                SELECT unnest($2::uuid[]) AS trace_id
            ),
            q AS (
                SELECT websearch_to_tsquery('simple', $3) AS query
            ),
            candidates AS (
                SELECT
                    tm.trace_id,
                    l.id AS landmark_id,
                    l.title,
                    l.subtitle,
                    l.landmark_type,
                    l.updated_at,
                    0 AS relation_priority
                FROM input_trace_ids ti
                INNER JOIN trace_mirrors tm
                  ON tm.trace_id = ti.trace_id
                INNER JOIN "references" r
                  ON r.trace_mirror_id = tm.id
                INNER JOIN landscape_analyses la
                  ON la.id = r.landscape_analysis_id
                 AND la.landscape_analysis_type = 'TRACE_INCREMENTAL'
                INNER JOIN users u
                  ON u.id = $1
                INNER JOIN lens_analysis_scopes las
                  ON las.landscape_analysis_id = r.landscape_analysis_id
                 AND las.lens_id = u.current_lens_id
                INNER JOIN landmarks l
                  ON l.id = r.landmark_id
                WHERE r.landmark_id IS NOT NULL
            ),
            grouped AS (
                SELECT
                    trace_id,
                    landmark_id,
                    title,
                    subtitle,
                    landmark_type,
                    MAX(updated_at) AS updated_at,
                    MIN(relation_priority) AS relation_priority
                FROM candidates
                GROUP BY trace_id, landmark_id, title, subtitle, landmark_type
            ),
            scored AS (
                SELECT
                    g.trace_id,
                    g.landmark_id,
                    g.title,
                    g.subtitle,
                    g.landmark_type = 'HIGH_LEVEL_PROJECT' AS is_high_level_project,
                    (
                        setweight(to_tsvector('simple', g.title), 'A') ||
                        setweight(to_tsvector('simple', g.subtitle), 'B')
                    ) @@ q.query AS matched_query,
                    ts_rank_cd(
                        setweight(to_tsvector('simple', g.title), 'A') ||
                        setweight(to_tsvector('simple', g.subtitle), 'B'),
                        q.query
                    )::real AS score,
                    g.relation_priority,
                    g.updated_at
                FROM grouped g
                CROSS JOIN q
            ),
            ranked AS (
                SELECT
                    trace_id,
                    landmark_id,
                    title,
                    subtitle,
                    score,
                    matched_query,
                    is_high_level_project,
                    ROW_NUMBER() OVER (
                        PARTITION BY trace_id, is_high_level_project
                        ORDER BY matched_query DESC, score DESC, relation_priority ASC, updated_at DESC
                    )::integer AS row_rank
                FROM scored
            )
            SELECT
                trace_id,
                landmark_id,
                title,
                subtitle,
                score,
                matched_query,
                is_high_level_project,
                row_rank
            FROM ranked
            WHERE row_rank <= 5
            ORDER BY trace_id ASC, is_high_level_project ASC, row_rank ASC
            "#,
        )
        .bind::<SqlUuid, _>(user_id)
        .bind::<Array<SqlUuid>, _>(trace_ids)
        .bind::<Text, _>(query)
        .load::<TraceSearchLandmarkRow>(&mut conn)?;

        let mut contexts: HashMap<Uuid, TraceSearchLandmarkContext> = HashMap::new();
        for row in rows {
            let context = contexts.entry(row.trace_id).or_default();
            let landmark = TraceSearchLandmark {
                id: row.landmark_id,
                title: row.title,
                subtitle: row.subtitle,
                score: row.score,
                matched_query: row.matched_query,
            };
            if row.is_high_level_project {
                context.high_level_project_landmarks.push(landmark);
            } else {
                context.landmarks.push(landmark);
            }
        }

        Ok(contexts)
    }
}

#[derive(Default)]
struct TraceSearchLandmarkContext {
    landmarks: Vec<TraceSearchLandmark>,
    high_level_project_landmarks: Vec<TraceSearchLandmark>,
}
