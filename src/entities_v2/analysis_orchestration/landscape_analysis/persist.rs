use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::Timestamp as SqlTimestamp;
use diesel::sql_types::Uuid as SqlUuid;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::reference::Reference;
use crate::entities_v2::{
    lens::Lens,
    trace::{Trace, TraceType},
    user::User,
};
use crate::schema::{
    landscape_analyses, landscape_analysis_inputs, landscape_landmarks, lens_analysis_scopes,
};

use super::model::{
    LandscapeAnalysis, LandscapeAnalysisInputType, LandscapeAnalysisType, LandscapeProcessingState,
    NewLandscapeAnalysis,
};

#[derive(diesel::QueryableByName)]
struct IdRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
}

impl LandscapeAnalysis {
    pub fn update(self, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        diesel::update(landscape_analyses::table.filter(landscape_analyses::id.eq(self.id)))
            .set((
                landscape_analyses::title.eq(self.title),
                landscape_analyses::subtitle.eq(self.subtitle),
                landscape_analyses::plain_text_state_summary.eq(self.plain_text_state_summary),
                landscape_analyses::interaction_date.eq(self.interaction_date),
                landscape_analyses::period_start.eq(self.period_start),
                landscape_analyses::period_end.eq(self.period_end),
                landscape_analyses::landscape_analysis_type
                    .eq(self.landscape_analysis_type.to_db()),
                landscape_analyses::processing_state.eq(self.processing_state.to_db()),
                landscape_analyses::parent_id.eq(self.parent_analysis_id),
                landscape_analyses::replayed_from_id.eq(self.replayed_from_id),
                landscape_analyses::analyzed_trace_id.eq(self.analyzed_trace_id),
                landscape_analyses::trace_mirror_id.eq(self.trace_mirror_id),
            ))
            .execute(&mut conn)?;
        LandscapeAnalysis::find_full_analysis(self.id, pool)
    }

    pub fn set_processing_state(
        mut self,
        state: LandscapeProcessingState,
        pool: &DbPool,
    ) -> Result<LandscapeAnalysis, PpdcError> {
        self.processing_state = state;
        self.update(pool)
    }

    pub fn delete(self, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        diesel::delete(landscape_analyses::table.filter(landscape_analyses::id.eq(self.id)))
            .execute(&mut conn)?;
        Ok(self)
    }
}

impl NewLandscapeAnalysis {
    pub fn create(self, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let id: Uuid = diesel::insert_into(landscape_analyses::table)
            .values((
                landscape_analyses::title.eq(self.title),
                landscape_analyses::subtitle.eq(self.subtitle),
                landscape_analyses::plain_text_state_summary.eq(self.plain_text_state_summary),
                landscape_analyses::interaction_date.eq(Some(self.interaction_date)),
                landscape_analyses::period_start.eq(self.period_start),
                landscape_analyses::period_end.eq(self.period_end),
                landscape_analyses::user_id.eq(self.user_id),
                landscape_analyses::landscape_analysis_type
                    .eq(self.landscape_analysis_type.to_db()),
                landscape_analyses::processing_state.eq(LandscapeProcessingState::Pending.to_db()),
                landscape_analyses::parent_id.eq(self.parent_analysis_id),
                landscape_analyses::analyzed_trace_id.eq(self.analyzed_trace_id),
                landscape_analyses::replayed_from_id.eq(self.replayed_from_id),
                landscape_analyses::trace_mirror_id.eq(self.trace_mirror_id),
            ))
            .returning(landscape_analyses::id)
            .get_result(&mut conn)?;

        LandscapeAnalysis::find_full_analysis(id, pool)
    }

    pub fn create_for_lens(
        self,
        lens_id: Uuid,
        pool: &DbPool,
    ) -> Result<LandscapeAnalysis, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let analysis_id: Uuid = conn.transaction::<Uuid, diesel::result::Error, _>(|conn| {
            let id: Uuid = diesel::insert_into(landscape_analyses::table)
                .values((
                    landscape_analyses::title.eq(self.title.clone()),
                    landscape_analyses::subtitle.eq(self.subtitle.clone()),
                    landscape_analyses::plain_text_state_summary
                        .eq(self.plain_text_state_summary.clone()),
                    landscape_analyses::interaction_date.eq(Some(self.interaction_date)),
                    landscape_analyses::period_start.eq(self.period_start),
                    landscape_analyses::period_end.eq(self.period_end),
                    landscape_analyses::user_id.eq(self.user_id),
                    landscape_analyses::landscape_analysis_type
                        .eq(self.landscape_analysis_type.to_db()),
                    landscape_analyses::processing_state
                        .eq(LandscapeProcessingState::Pending.to_db()),
                    landscape_analyses::parent_id.eq(self.parent_analysis_id),
                    landscape_analyses::analyzed_trace_id.eq(self.analyzed_trace_id),
                    landscape_analyses::replayed_from_id.eq(self.replayed_from_id),
                    landscape_analyses::trace_mirror_id.eq(self.trace_mirror_id),
                ))
                .returning(landscape_analyses::id)
                .get_result(conn)?;

            diesel::insert_into(lens_analysis_scopes::table)
                .values((
                    lens_analysis_scopes::id.eq(Uuid::new_v4()),
                    lens_analysis_scopes::lens_id.eq(lens_id),
                    lens_analysis_scopes::landscape_analysis_id.eq(id),
                ))
                .on_conflict((
                    lens_analysis_scopes::lens_id,
                    lens_analysis_scopes::landscape_analysis_id,
                ))
                .do_nothing()
                .execute(conn)?;
            Ok(id)
        })?;

        LandscapeAnalysis::find_full_analysis(analysis_id, pool)
    }
}

fn local_midnight(tz: Tz, date: NaiveDate) -> Result<DateTime<Tz>, PpdcError> {
    let naive_midnight = date.and_hms_opt(0, 0, 0).ok_or_else(|| {
        PpdcError::new(
            500,
            ErrorType::InternalError,
            "Failed to construct local midnight".to_string(),
        )
    })?;

    tz.from_local_datetime(&naive_midnight)
        .single()
        .or_else(|| tz.from_local_datetime(&naive_midnight).earliest())
        .or_else(|| tz.from_local_datetime(&naive_midnight).latest())
        .ok_or_else(|| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                "Failed to resolve local midnight for timezone".to_string(),
            )
        })
}

fn parse_user_timezone_or_utc(user: &User) -> Tz {
    user.timezone.parse::<Tz>().unwrap_or(chrono_tz::UTC)
}

fn trace_effective_datetime(trace: &Trace) -> NaiveDateTime {
    trace.interaction_date
}

fn day_period_for_datetime(
    datetime_utc: NaiveDateTime,
    tz: Tz,
) -> Result<(NaiveDateTime, NaiveDateTime), PpdcError> {
    let utc_dt = DateTime::<Utc>::from_naive_utc_and_offset(datetime_utc, Utc);
    let local_dt = utc_dt.with_timezone(&tz);
    let local_day_start = local_midnight(tz, local_dt.date_naive())?;
    let local_day_end = local_day_start + Duration::days(1);

    Ok((
        local_day_start.with_timezone(&Utc).naive_utc(),
        local_day_end.with_timezone(&Utc).naive_utc(),
    ))
}

fn week_period_for_datetime(
    datetime_utc: NaiveDateTime,
    tz: Tz,
    week_start_weekday: chrono::Weekday,
) -> Result<(NaiveDateTime, NaiveDateTime), PpdcError> {
    let utc_dt = DateTime::<Utc>::from_naive_utc_and_offset(datetime_utc, Utc);
    let local_dt = utc_dt.with_timezone(&tz);
    let local_date = local_dt.date_naive();

    let current = i64::from(local_date.weekday().num_days_from_monday());
    let target = i64::from(week_start_weekday.num_days_from_monday());
    let delta_days = (7 + current - target) % 7;
    let week_start_date = local_date - Duration::days(delta_days);

    let local_week_start = local_midnight(tz, week_start_date)?;
    let local_week_end = local_week_start + Duration::days(7);

    Ok((
        local_week_start.with_timezone(&Utc).naive_utc(),
        local_week_end.with_timezone(&Utc).naive_utc(),
    ))
}

fn find_analysis_covering_moment_for_lens(
    lens_id: Uuid,
    analysis_type: LandscapeAnalysisType,
    moment: NaiveDateTime,
    pool: &DbPool,
) -> Result<Option<LandscapeAnalysis>, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let maybe_id = lens_analysis_scopes::table
        .inner_join(
            landscape_analyses::table
                .on(lens_analysis_scopes::landscape_analysis_id.eq(landscape_analyses::id)),
        )
        .filter(lens_analysis_scopes::lens_id.eq(lens_id))
        .filter(landscape_analyses::landscape_analysis_type.eq(analysis_type.to_db()))
        .filter(landscape_analyses::period_start.le(moment))
        .filter(landscape_analyses::period_end.gt(moment))
        .select(landscape_analyses::id)
        .first::<Uuid>(&mut conn)
        .optional()?;

    match maybe_id {
        Some(id) => Ok(Some(LandscapeAnalysis::find_full_analysis(id, pool)?)),
        None => Ok(None),
    }
}

fn has_trace_analysis_for_lens(
    lens_id: Uuid,
    analysis_type: LandscapeAnalysisType,
    analyzed_trace_id: Uuid,
    pool: &DbPool,
) -> Result<bool, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let maybe_id = lens_analysis_scopes::table
        .inner_join(
            landscape_analyses::table
                .on(lens_analysis_scopes::landscape_analysis_id.eq(landscape_analyses::id)),
        )
        .filter(lens_analysis_scopes::lens_id.eq(lens_id))
        .filter(landscape_analyses::landscape_analysis_type.eq(analysis_type.to_db()))
        .filter(landscape_analyses::analyzed_trace_id.eq(Some(analyzed_trace_id)))
        .select(landscape_analyses::id)
        .first::<Uuid>(&mut conn)
        .optional()?;

    Ok(maybe_id.is_some())
}

pub fn create_for_trace_and_lens(
    trace_id: Uuid,
    lens_id: Uuid,
    pool: &DbPool,
) -> Result<Vec<LandscapeAnalysis>, PpdcError> {
    create_for_trace_and_lens_with_options(trace_id, lens_id, false, pool)
}

pub fn create_for_trace_and_lens_with_options(
    trace_id: Uuid,
    lens_id: Uuid,
    use_anchor_for_context_types: bool,
    pool: &DbPool,
) -> Result<Vec<LandscapeAnalysis>, PpdcError> {
    create_for_trace_and_lens_with_options_and_anchor(
        trace_id,
        lens_id,
        use_anchor_for_context_types,
        None,
        pool,
    )
}

pub fn create_for_trace_and_lens_with_options_and_anchor(
    trace_id: Uuid,
    lens_id: Uuid,
    use_anchor_for_context_types: bool,
    anchor_override: Option<NaiveDateTime>,
    pool: &DbPool,
) -> Result<Vec<LandscapeAnalysis>, PpdcError> {
    let lens = Lens::find_full_lens(lens_id, pool)?;
    let user_id = lens.user_id.ok_or_else(|| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            "Lens user_id is missing".to_string(),
        )
    })?;
    let user = User::find(&user_id, pool)?;
    let trace = Trace::find_full_trace(trace_id, pool)?;
    let trace_datetime = trace_effective_datetime(&trace);

    let (trace_analysis_type, analysis_datetime) = match trace.trace_type {
        TraceType::HighLevelProjectsDefinition => {
            let dt = if use_anchor_for_context_types {
                anchor_override
                    .or(user.context_anchor_at)
                    .unwrap_or(trace_datetime)
                    - Duration::days(1)
            } else {
                trace_datetime
            };
            (LandscapeAnalysisType::Hlp, dt)
        }
        TraceType::BioTrace => {
            let dt = if use_anchor_for_context_types {
                anchor_override
                    .or(user.context_anchor_at)
                    .unwrap_or(trace_datetime)
                    - Duration::days(1)
            } else {
                trace_datetime
            };
            (LandscapeAnalysisType::Bio, dt)
        }
        _ => (LandscapeAnalysisType::TraceIncremental, trace_datetime),
    };

    let mut created = Vec::new();

    if !has_trace_analysis_for_lens(lens_id, trace_analysis_type, trace.id, pool)? {
        let new_analysis = match trace_analysis_type {
            LandscapeAnalysisType::Hlp => NewLandscapeAnalysis::new_hlp(
                format!("HLP analysis {}", trace.id),
                String::new(),
                String::new(),
                user_id,
                analysis_datetime,
                None,
                Some(trace.id),
                None,
            ),
            LandscapeAnalysisType::Bio => NewLandscapeAnalysis::new_bio(
                format!("Bio analysis {}", trace.id),
                String::new(),
                String::new(),
                user_id,
                analysis_datetime,
                None,
                Some(trace.id),
                None,
            ),
            _ => NewLandscapeAnalysis::new_trace_incremental(
                format!("Trace analysis {}", trace.id),
                String::new(),
                String::new(),
                user_id,
                analysis_datetime,
                None,
                Some(trace.id),
                None,
            ),
        };
        created.push(new_analysis.create_for_lens(lens_id, pool)?);
    }

    if matches!(
        trace.trace_type,
        TraceType::UserTrace | TraceType::WorkspaceTrace
    ) {
        let tz = parse_user_timezone_or_utc(&user);

        if let Some(existing_daily) = find_analysis_covering_moment_for_lens(
            lens_id,
            LandscapeAnalysisType::DailyRecap,
            trace_datetime,
            pool,
        )? {
            let _ = replace_covered_inputs_for_period(
                existing_daily.id,
                lens_id,
                user_id,
                existing_daily.period_start,
                existing_daily.period_end,
                pool,
            )?;
        } else {
            let (day_start, day_end) = day_period_for_datetime(trace_datetime, tz)?;
            let daily = NewLandscapeAnalysis::new_daily_recap(
                format!("Daily recap {}", day_start.date()),
                String::new(),
                String::new(),
                user_id,
                trace_datetime,
                day_start,
                day_end,
                None,
                None,
            )
            .create_for_lens(lens_id, pool)?;
            let _ = replace_covered_inputs_for_period(
                daily.id, lens_id, user_id, day_start, day_end, pool,
            )?;
            created.push(daily);
        }

        if let Some(existing_weekly) = find_analysis_covering_moment_for_lens(
            lens_id,
            LandscapeAnalysisType::WeeklyRecap,
            trace_datetime,
            pool,
        )? {
            let _ = replace_covered_inputs_for_period(
                existing_weekly.id,
                lens_id,
                user_id,
                existing_weekly.period_start,
                existing_weekly.period_end,
                pool,
            )?;
        } else {
            let week_start_weekday = user.week_analysis_weekday.to_chrono_weekday();
            let (week_start, week_end) =
                week_period_for_datetime(trace_datetime, tz, week_start_weekday)?;
            let weekly = NewLandscapeAnalysis::new_weekly_recap(
                format!("Weekly recap {}", week_start.date()),
                String::new(),
                String::new(),
                user_id,
                trace_datetime,
                week_start,
                week_end,
                None,
                None,
            )
            .create_for_lens(lens_id, pool)?;
            let _ = replace_covered_inputs_for_period(
                weekly.id, lens_id, user_id, week_start, week_end, pool,
            )?;
            created.push(weekly);
        }
    }

    Ok(created)
}

pub fn refresh_pending_summary_covered_inputs_for_trace(
    lens_id: Uuid,
    user_id: Uuid,
    trace_datetime: NaiveDateTime,
    pool: &DbPool,
) -> Result<(usize, usize, usize), PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let pending_summary_rows = lens_analysis_scopes::table
        .inner_join(
            landscape_analyses::table
                .on(lens_analysis_scopes::landscape_analysis_id.eq(landscape_analyses::id)),
        )
        .filter(lens_analysis_scopes::lens_id.eq(lens_id))
        .filter(landscape_analyses::user_id.eq(user_id))
        .filter(landscape_analyses::landscape_analysis_type.eq_any(vec![
            LandscapeAnalysisType::DailyRecap.to_db(),
            LandscapeAnalysisType::WeeklyRecap.to_db(),
        ]))
        .filter(landscape_analyses::processing_state.eq(LandscapeProcessingState::Pending.to_db()))
        .filter(landscape_analyses::period_start.le(trace_datetime))
        .filter(landscape_analyses::period_end.gt(trace_datetime))
        .select((
            landscape_analyses::id,
            landscape_analyses::period_start,
            landscape_analyses::period_end,
        ))
        .load::<(Uuid, NaiveDateTime, NaiveDateTime)>(&mut conn)?;

    let mut refreshed_analyses = 0usize;
    let mut refreshed_traces = 0usize;
    let mut refreshed_trace_mirrors = 0usize;

    for (analysis_id, period_start, period_end) in pending_summary_rows {
        let (trace_count, trace_mirror_count) = replace_covered_inputs_for_period(
            analysis_id,
            lens_id,
            user_id,
            period_start,
            period_end,
            pool,
        )?;
        refreshed_analyses += 1;
        refreshed_traces += trace_count;
        refreshed_trace_mirrors += trace_mirror_count;
    }

    Ok((
        refreshed_analyses,
        refreshed_traces,
        refreshed_trace_mirrors,
    ))
}

pub fn claim_next_pending_for_lens(
    lens_id: Uuid,
    claim_cutoff_at: NaiveDateTime,
    pool: &DbPool,
) -> Result<Option<LandscapeAnalysis>, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let claimed = sql_query(
        r#"
WITH candidate AS (
    SELECT la.id
    FROM landscape_analyses la
    INNER JOIN lens_analysis_scopes las
        ON las.landscape_analysis_id = la.id
    WHERE las.lens_id = $1
      AND la.processing_state = 'PENDING'
      AND la.period_end <= $2
      AND (
        la.landscape_analysis_type NOT IN ('DAILY_RECAP', 'WEEKLY_RECAP')
        OR NOT EXISTS (
            SELECT 1
            FROM traces t
            WHERE t.user_id = la.user_id
              AND t.interaction_date >= la.period_start
              AND t.interaction_date < la.period_end
              AND NOT EXISTS (
                  SELECT 1
                  FROM trace_mirrors tm
                  INNER JOIN lens_analysis_scopes las_tm
                      ON las_tm.landscape_analysis_id = tm.landscape_analysis_id
                  WHERE las_tm.lens_id = las.lens_id
                    AND tm.trace_id = t.id
              )
        )
      )
    ORDER BY
      la.period_end ASC,
      CASE la.landscape_analysis_type
        WHEN 'DAILY_RECAP' THEN 0
        WHEN 'WEEKLY_RECAP' THEN 1
        ELSE 0
      END ASC,
      la.created_at ASC
    LIMIT 1
    FOR UPDATE SKIP LOCKED
)
UPDATE landscape_analyses la
SET processing_state = 'RUNNING',
    updated_at = NOW()
FROM candidate
WHERE la.id = candidate.id
RETURNING la.id
        "#,
    )
    .bind::<SqlUuid, _>(lens_id)
    .bind::<diesel::sql_types::Timestamp, _>(claim_cutoff_at)
    .get_result::<IdRow>(&mut conn)
    .optional()?;

    match claimed {
        Some(row) => Ok(Some(LandscapeAnalysis::find_full_analysis(row.id, pool)?)),
        None => Ok(None),
    }
}

pub fn delete_leaf_and_cleanup(
    id: Uuid,
    pool: &DbPool,
) -> Result<Option<LandscapeAnalysis>, PpdcError> {
    let landscape_analysis = LandscapeAnalysis::find_full_analysis(id, pool)?;
    let children_landscape_analyses = landscape_analysis.get_children_landscape_analyses(pool)?;
    let heading_lens = landscape_analysis.get_heading_lens(pool)?;
    if !children_landscape_analyses.is_empty() || !heading_lens.is_empty() {
        return Ok(None);
    }

    Reference::delete_for_landscape_analysis(id, pool)?;
    let deleted = landscape_analysis.delete(pool)?;
    Ok(Some(deleted))
}

pub fn find_last_analysis_resource(
    user_id: Uuid,
    pool: &DbPool,
) -> Result<Option<LandscapeAnalysis>, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");
    let maybe_id = landscape_analyses::table
        .filter(landscape_analyses::user_id.eq(user_id))
        .order(landscape_analyses::interaction_date.desc().nulls_last())
        .then_order_by(landscape_analyses::created_at.desc())
        .select(landscape_analyses::id)
        .first::<Uuid>(&mut conn)
        .optional()?;
    match maybe_id {
        Some(id) => Ok(Some(LandscapeAnalysis::find_full_analysis(id, pool)?)),
        None => Ok(None),
    }
}

fn add_trace_input_with_conn(
    conn: &mut diesel::PgConnection,
    landscape_analysis_id: Uuid,
    trace_id: Uuid,
    input_type: LandscapeAnalysisInputType,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(landscape_analysis_inputs::table)
        .values((
            landscape_analysis_inputs::landscape_analysis_id.eq(landscape_analysis_id),
            landscape_analysis_inputs::trace_id.eq(Some(trace_id)),
            landscape_analysis_inputs::trace_mirror_id.eq(None::<Uuid>),
            landscape_analysis_inputs::input_type.eq(input_type.to_db()),
        ))
        .on_conflict_do_nothing()
        .execute(conn)?;
    Ok(())
}

fn add_trace_mirror_input_with_conn(
    conn: &mut diesel::PgConnection,
    landscape_analysis_id: Uuid,
    trace_mirror_id: Uuid,
    input_type: LandscapeAnalysisInputType,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(landscape_analysis_inputs::table)
        .values((
            landscape_analysis_inputs::landscape_analysis_id.eq(landscape_analysis_id),
            landscape_analysis_inputs::trace_id.eq(None::<Uuid>),
            landscape_analysis_inputs::trace_mirror_id.eq(Some(trace_mirror_id)),
            landscape_analysis_inputs::input_type.eq(input_type.to_db()),
        ))
        .on_conflict_do_nothing()
        .execute(conn)?;
    Ok(())
}

pub fn add_trace_input(
    landscape_analysis_id: Uuid,
    trace_id: Uuid,
    input_type: LandscapeAnalysisInputType,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");
    add_trace_input_with_conn(&mut conn, landscape_analysis_id, trace_id, input_type)?;
    Ok(())
}

pub fn add_trace_mirror_input(
    landscape_analysis_id: Uuid,
    trace_mirror_id: Uuid,
    input_type: LandscapeAnalysisInputType,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");
    add_trace_mirror_input_with_conn(
        &mut conn,
        landscape_analysis_id,
        trace_mirror_id,
        input_type,
    )?;
    Ok(())
}

pub fn replace_covered_inputs_for_period(
    landscape_analysis_id: Uuid,
    lens_id: Uuid,
    user_id: Uuid,
    period_start: NaiveDateTime,
    period_end: NaiveDateTime,
    pool: &DbPool,
) -> Result<(usize, usize), PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let counts = conn.transaction::<(usize, usize), diesel::result::Error, _>(|conn| {
        diesel::delete(
            landscape_analysis_inputs::table
                .filter(landscape_analysis_inputs::landscape_analysis_id.eq(landscape_analysis_id))
                .filter(
                    landscape_analysis_inputs::input_type
                        .eq(LandscapeAnalysisInputType::Covered.to_db()),
                ),
        )
        .execute(conn)?;

        let covered_trace_rows = sql_query(
            r#"
SELECT t.id
FROM traces t
WHERE t.user_id = $1
  AND t.interaction_date >= $2
  AND t.interaction_date < $3
ORDER BY t.interaction_date ASC, t.created_at ASC
            "#,
        )
        .bind::<SqlUuid, _>(user_id)
        .bind::<SqlTimestamp, _>(period_start)
        .bind::<SqlTimestamp, _>(period_end)
        .load::<IdRow>(conn)?;

        let mut inserted_traces = 0usize;
        for row in covered_trace_rows {
            add_trace_input_with_conn(
                conn,
                landscape_analysis_id,
                row.id,
                LandscapeAnalysisInputType::Covered,
            )?;
            inserted_traces += 1;
        }

        let covered_trace_mirror_rows = sql_query(
            r#"
SELECT DISTINCT tm.id
FROM trace_mirrors tm
INNER JOIN lens_analysis_scopes las
    ON las.landscape_analysis_id = tm.landscape_analysis_id
INNER JOIN traces t
    ON t.id = tm.trace_id
WHERE las.lens_id = $1
  AND t.user_id = $2
  AND t.interaction_date >= $3
  AND t.interaction_date < $4
ORDER BY tm.id ASC
            "#,
        )
        .bind::<SqlUuid, _>(lens_id)
        .bind::<SqlUuid, _>(user_id)
        .bind::<SqlTimestamp, _>(period_start)
        .bind::<SqlTimestamp, _>(period_end)
        .load::<IdRow>(conn)?;

        let mut inserted_trace_mirrors = 0usize;
        for row in covered_trace_mirror_rows {
            add_trace_mirror_input_with_conn(
                conn,
                landscape_analysis_id,
                row.id,
                LandscapeAnalysisInputType::Covered,
            )?;
            inserted_trace_mirrors += 1;
        }

        Ok((inserted_traces, inserted_trace_mirrors))
    })?;

    Ok(counts)
}

pub fn add_landmark_ref(
    landscape_analysis_id: Uuid,
    landmark_id: Uuid,
    _user_id: Uuid,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");
    diesel::insert_into(landscape_landmarks::table)
        .values((
            landscape_landmarks::landscape_analysis_id.eq(landscape_analysis_id),
            landscape_landmarks::landmark_id.eq(landmark_id),
            landscape_landmarks::relation_type.eq("REFERENCED"),
        ))
        .on_conflict((
            landscape_landmarks::landscape_analysis_id,
            landscape_landmarks::landmark_id,
            landscape_landmarks::relation_type,
        ))
        .do_nothing()
        .execute(&mut conn)?;
    Ok(())
}

pub fn copy_landmark_links_from_analysis(
    source_landscape_analysis_id: Uuid,
    target_landscape_analysis_id: Uuid,
    pool: &DbPool,
) -> Result<usize, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let source_landmark_ids = landscape_landmarks::table
        .filter(landscape_landmarks::landscape_analysis_id.eq(source_landscape_analysis_id))
        .select(landscape_landmarks::landmark_id)
        .distinct()
        .load::<Uuid>(&mut conn)?;

    let mut inserted = 0usize;
    for landmark_id in source_landmark_ids {
        let rows = diesel::insert_into(landscape_landmarks::table)
            .values((
                landscape_landmarks::landscape_analysis_id.eq(target_landscape_analysis_id),
                landscape_landmarks::landmark_id.eq(landmark_id),
                landscape_landmarks::relation_type.eq("REFERENCED"),
            ))
            .on_conflict((
                landscape_landmarks::landscape_analysis_id,
                landscape_landmarks::landmark_id,
                landscape_landmarks::relation_type,
            ))
            .do_nothing()
            .execute(&mut conn)?;
        inserted += rows as usize;
    }

    Ok(inserted)
}
