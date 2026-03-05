use chrono::{Duration, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::{
    landscape_analysis::{
        self, create_for_trace_and_lens_with_options,
        create_for_trace_and_lens_with_options_and_anchor, LandscapeAnalysis, NewLandscapeAnalysis,
    },
    trace::{Trace, TraceType},
};
use crate::schema::{lens_analysis_scopes, lens_heads, lens_targets, lenses};

use super::model::{Lens, LensProcessingState, NewLens};

fn ensure_lens_analysis_scope_relation(
    lens_id: Uuid,
    analysis_id: Uuid,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");
    diesel::insert_into(lens_analysis_scopes::table)
        .values((
            lens_analysis_scopes::id.eq(Uuid::new_v4()),
            lens_analysis_scopes::lens_id.eq(lens_id),
            lens_analysis_scopes::landscape_analysis_id.eq(analysis_id),
        ))
        .on_conflict((
            lens_analysis_scopes::lens_id,
            lens_analysis_scopes::landscape_analysis_id,
        ))
        .do_nothing()
        .execute(&mut conn)?;
    Ok(())
}

fn remove_lens_analysis_scope_relation(
    lens_id: Uuid,
    analysis_id: Uuid,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");
    diesel::delete(
        lens_analysis_scopes::table
            .filter(lens_analysis_scopes::lens_id.eq(lens_id))
            .filter(lens_analysis_scopes::landscape_analysis_id.eq(analysis_id)),
    )
    .execute(&mut conn)?;
    Ok(())
}

fn enqueue_pending_analyses_for_lens_target(
    lens_id: Uuid,
    user_id: Uuid,
    target_trace_id: Uuid,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let all_user_traces = Trace::get_all_for_user(user_id, pool)?;
    let trace_effective_date = |trace: &Trace| trace.interaction_date.unwrap_or(trace.created_at);

    let anchor_date = all_user_traces
        .iter()
        .filter(|trace| {
            matches!(
                trace.trace_type,
                TraceType::UserTrace | TraceType::WorkspaceTrace
            )
        })
        .map(trace_effective_date)
        .min()
        .or_else(|| all_user_traces.iter().map(trace_effective_date).min());

    let latest_hlp_trace = all_user_traces
        .iter()
        .filter(|trace| trace.trace_type == TraceType::HighLevelProjectsDefinition)
        .max_by_key(|trace| trace_effective_date(trace))
        .map(|trace| trace.id);
    if let Some(hlp_trace_id) = latest_hlp_trace {
        let _ = create_for_trace_and_lens_with_options_and_anchor(
            hlp_trace_id,
            lens_id,
            true,
            anchor_date,
            pool,
        )?;
    }

    let latest_bio_trace = all_user_traces
        .iter()
        .filter(|trace| trace.trace_type == TraceType::BioTrace)
        .max_by_key(|trace| trace_effective_date(trace))
        .map(|trace| trace.id);
    if let Some(bio_trace_id) = latest_bio_trace {
        let _ = create_for_trace_and_lens_with_options_and_anchor(
            bio_trace_id,
            lens_id,
            true,
            anchor_date,
            pool,
        )?;
    }

    let traces = Trace::get_before(user_id, target_trace_id, pool)?;
    for trace in traces.iter().filter(|trace| {
        !matches!(
            trace.trace_type,
            TraceType::HighLevelProjectsDefinition | TraceType::BioTrace
        )
    }) {
        let _ = create_for_trace_and_lens_with_options(trace.id, lens_id, true, pool)?;
    }

    Ok(())
}

impl Lens {
    pub fn try_acquire_run_lock(
        &self,
        worker_id: Uuid,
        ttl_seconds: i64,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
        let now = Utc::now().naive_utc();
        let lock_until = now + Duration::seconds(ttl_seconds.max(1));
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let updated_rows = diesel::update(
            lenses::table.filter(lenses::id.eq(self.id)).filter(
                lenses::run_lock_until
                    .is_null()
                    .or(lenses::run_lock_until.lt(now))
                    .or(lenses::run_lock_owner.eq(Some(worker_id))),
            ),
        )
        .set((
            lenses::run_lock_owner.eq(Some(worker_id)),
            lenses::run_lock_until.eq(Some(lock_until)),
        ))
        .execute(&mut conn)?;

        Ok(updated_rows == 1)
    }

    pub fn renew_run_lock(
        &self,
        worker_id: Uuid,
        ttl_seconds: i64,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
        let now = Utc::now().naive_utc();
        let lock_until = now + Duration::seconds(ttl_seconds.max(1));
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let updated_rows = diesel::update(
            lenses::table
                .filter(lenses::id.eq(self.id))
                .filter(lenses::run_lock_owner.eq(Some(worker_id))),
        )
        .set(lenses::run_lock_until.eq(Some(lock_until)))
        .execute(&mut conn)?;

        Ok(updated_rows == 1)
    }

    pub fn release_run_lock(&self, worker_id: Uuid, pool: &DbPool) -> Result<bool, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let updated_rows = diesel::update(
            lenses::table
                .filter(lenses::id.eq(self.id))
                .filter(lenses::run_lock_owner.eq(Some(worker_id))),
        )
        .set((
            lenses::run_lock_owner.eq::<Option<Uuid>>(None),
            lenses::run_lock_until.eq::<Option<chrono::NaiveDateTime>>(None),
        ))
        .execute(&mut conn)?;

        Ok(updated_rows == 1)
    }

    pub fn delete(self, pool: &DbPool) -> Result<Lens, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        diesel::delete(lenses::table.filter(lenses::id.eq(self.id))).execute(&mut conn)?;
        Ok(self)
    }

    pub fn update_current_landscape(
        self,
        new_landscape_analysis_id: Uuid,
        pool: &DbPool,
    ) -> Result<Lens, PpdcError> {
        let new_landscape = LandscapeAnalysis::find_full_analysis(new_landscape_analysis_id, pool)?;
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        diesel::update(lenses::table.filter(lenses::id.eq(self.id)))
            .set(lenses::current_landscape_id.eq(Some(new_landscape_analysis_id)))
            .execute(&mut conn)?;

        diesel::insert_into(lens_heads::table)
            .values((
                lens_heads::id.eq(Uuid::new_v4()),
                lens_heads::lens_id.eq(self.id),
                lens_heads::landscape_analysis_id.eq(new_landscape_analysis_id),
            ))
            .on_conflict(lens_heads::lens_id)
            .do_update()
            .set(lens_heads::landscape_analysis_id.eq(new_landscape_analysis_id))
            .execute(&mut conn)?;

        if let Some(replayed_analysis_id) = new_landscape.replayed_from_id {
            remove_lens_analysis_scope_relation(self.id, replayed_analysis_id, pool)?;
        }
        ensure_lens_analysis_scope_relation(self.id, new_landscape_analysis_id, pool)?;

        Lens::find_full_lens(self.id, pool)
    }

    pub fn update_target_trace(
        self,
        new_target_trace_id: Option<Uuid>,
        pool: &DbPool,
    ) -> Result<Lens, PpdcError> {
        let user_id = self.user_id.ok_or_else(|| {
            PpdcError::new(
                400,
                ErrorType::ApiError,
                "Lens user_id is missing".to_string(),
            )
        })?;
        let target_changed = self.target_trace_id != new_target_trace_id;

        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        diesel::update(lenses::table.filter(lenses::id.eq(self.id)))
            .set(lenses::target_trace_id.eq(new_target_trace_id))
            .execute(&mut conn)?;

        if let Some(new_target_trace_id) = new_target_trace_id {
            diesel::insert_into(lens_targets::table)
                .values((
                    lens_targets::id.eq(Uuid::new_v4()),
                    lens_targets::lens_id.eq(self.id),
                    lens_targets::trace_id.eq(new_target_trace_id),
                ))
                .on_conflict(lens_targets::lens_id)
                .do_update()
                .set(lens_targets::trace_id.eq(new_target_trace_id))
                .execute(&mut conn)?;
        } else {
            diesel::delete(lens_targets::table.filter(lens_targets::lens_id.eq(self.id)))
                .execute(&mut conn)?;
        }

        if target_changed {
            diesel::update(lenses::table.filter(lenses::id.eq(self.id)))
                .set(lenses::processing_state.eq(LensProcessingState::OutOfSync.to_db()))
                .execute(&mut conn)?;
            if let Some(next_target_trace_id) = new_target_trace_id {
                enqueue_pending_analyses_for_lens_target(
                    self.id,
                    user_id,
                    next_target_trace_id,
                    pool,
                )?;
            }
        }
        Lens::find_full_lens(self.id, pool)
    }

    pub fn set_processing_state(
        self,
        new_processing_state: LensProcessingState,
        pool: &DbPool,
    ) -> Result<Lens, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        diesel::update(lenses::table.filter(lenses::id.eq(self.id)))
            .set(lenses::processing_state.eq(new_processing_state.to_db()))
            .execute(&mut conn)?;
        Lens::find_full_lens(self.id, pool)
    }
}

impl NewLens {
    pub fn create(self, pool: &DbPool) -> Result<Lens, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let id = Uuid::new_v4();
        diesel::insert_into(lenses::table)
            .values((
                lenses::id.eq(id),
                lenses::user_id.eq(self.user_id),
                lenses::processing_state.eq(self.processing_state.to_db()),
                lenses::fork_landscape_id.eq(self.fork_landscape_id),
                lenses::current_landscape_id.eq(self.current_landscape_id),
                lenses::target_trace_id.eq(self.target_trace_id),
                lenses::autoplay.eq(self.autoplay),
            ))
            .execute(&mut conn)?;

        if let Some(current_landscape_id) = self.current_landscape_id {
            diesel::insert_into(lens_heads::table)
                .values((
                    lens_heads::id.eq(Uuid::new_v4()),
                    lens_heads::lens_id.eq(id),
                    lens_heads::landscape_analysis_id.eq(current_landscape_id),
                ))
                .on_conflict(lens_heads::lens_id)
                .do_update()
                .set(lens_heads::landscape_analysis_id.eq(current_landscape_id))
                .execute(&mut conn)?;
            ensure_lens_analysis_scope_relation(id, current_landscape_id, pool)?;
        }
        if let Some(target_trace_id) = self.target_trace_id {
            diesel::insert_into(lens_targets::table)
                .values((
                    lens_targets::id.eq(Uuid::new_v4()),
                    lens_targets::lens_id.eq(id),
                    lens_targets::trace_id.eq(target_trace_id),
                ))
                .on_conflict(lens_targets::lens_id)
                .do_update()
                .set(lens_targets::trace_id.eq(target_trace_id))
                .execute(&mut conn)?;
            enqueue_pending_analyses_for_lens_target(id, self.user_id, target_trace_id, pool)?;
        }
        Lens::find_full_lens(id, pool)
    }
}

pub fn create_landscape_placeholders(
    current_trace_id: Uuid,
    previous_landscape_analysis_id: Uuid,
    pool: &DbPool,
) -> Result<Vec<Uuid>, PpdcError> {
    let previous_landscape =
        LandscapeAnalysis::find_full_analysis(previous_landscape_analysis_id, pool)?;
    let user_id = previous_landscape.user_id;
    let traces: Vec<Trace>;
    if previous_landscape.analyzed_trace_id.is_none() {
        traces = Trace::get_before(user_id, current_trace_id, pool)?;
    } else {
        traces = Trace::get_between(
            user_id,
            previous_landscape.analyzed_trace_id.unwrap(),
            current_trace_id,
            pool,
        )?;
    }
    let mut landscape_analysis_ids = vec![previous_landscape_analysis_id];
    let mut previous_id = previous_landscape_analysis_id;
    for trace in traces {
        let new_landscape_analysis =
            NewLandscapeAnalysis::new_placeholder(user_id, trace.id, Some(previous_id), None)
                .create(pool)?;
        landscape_analysis_ids.push(new_landscape_analysis.id);
        previous_id = new_landscape_analysis.id;
    }
    Ok(landscape_analysis_ids)
}

pub fn delete_lens_and_landscapes(id: Uuid, pool: &DbPool) -> Result<Lens, PpdcError> {
    let mut lens = Lens::find_full_lens(id, pool)?;
    let mut landscape_analysis_id = lens.current_landscape_id;

    while let Some(id) = landscape_analysis_id {
        let current_landscape_analysis = LandscapeAnalysis::find_full_analysis(id, pool)?;
        if let Some(parent_analysis_id) = current_landscape_analysis.parent_analysis_id {
            lens = lens.update_current_landscape(parent_analysis_id, pool)?;
        } else {
            lens = lens.delete(pool)?;
            landscape_analysis::delete_leaf_and_cleanup(current_landscape_analysis.id, pool)?;
            return Ok(lens);
        }
        let deletion_option =
            landscape_analysis::delete_leaf_and_cleanup(current_landscape_analysis.id, pool)?;
        if deletion_option.is_some() {
            landscape_analysis_id = current_landscape_analysis.parent_analysis_id;
        } else {
            break;
        }
    }
    lens.delete(pool)
}
