use uuid::Uuid;

use crate::db::{get_global_pool, DbPool};
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::landscape_analysis::model::{
    LandscapeAnalysisInputType, LandscapeAnalysisType,
};
use crate::entities_v2::{
    landscape_analysis::{
        add_trace_input, add_trace_mirror_input, claim_next_pending_for_lens,
        copy_landmark_links_from_analysis, refresh_pending_summary_covered_inputs_for_trace,
        LandscapeAnalysis, LandscapeProcessingState,
    },
    lens::{Lens, LensProcessingState},
    trace::Trace,
};
use crate::work_analyzer::analysis_processor;
use crate::work_analyzer::period_analysis_processor;

const LENS_RUN_LOCK_TTL_SECONDS: i64 = 1800;
const MAX_ANALYSES_PER_RUN: usize = 100;

pub async fn run_lens(lens_id: Uuid) -> Result<Lens, PpdcError> {
    let pool = get_global_pool();
    let mut lens = Lens::find_full_lens(lens_id, pool)?;
    let worker_id = Uuid::new_v4();
    let claim_cutoff_at = resolve_claim_cutoff_at(&lens, pool)?;
    tracing::info!(
        target: "work_analyzer",
        "run_lens_start lens_id={} worker_id={} current_landscape_id={:?} target_trace_id={:?} claim_cutoff_at={}",
        lens.id,
        worker_id,
        lens.current_landscape_id,
        lens.target_trace_id,
        claim_cutoff_at
    );

    let acquired = lens.try_acquire_run_lock(worker_id, LENS_RUN_LOCK_TTL_SECONDS, pool)?;
    if !acquired {
        tracing::info!(
            target: "work_analyzer",
            "run_lens_lock_not_acquired lens_id={} worker_id={}",
            lens.id,
            worker_id
        );
        return Ok(lens);
    }
    tracing::info!(
        target: "work_analyzer",
        "run_lens_lock_acquired lens_id={} worker_id={}",
        lens.id,
        worker_id
    );

    let run_result = run_claim_loop(&mut lens, worker_id, claim_cutoff_at, pool).await;

    let release_result = lens.release_run_lock(worker_id, pool);
    match (run_result, release_result) {
        (Ok(()), Ok(_)) => {
            tracing::info!(
                target: "work_analyzer",
                "run_lens_complete lens_id={} worker_id={} current_landscape_id={:?}",
                lens.id,
                worker_id,
                lens.current_landscape_id
            );
            Ok(lens)
        }
        (Err(err), Ok(_)) => {
            if let Err(state_err) = lens
                .clone()
                .set_processing_state(LensProcessingState::Failed, pool)
            {
                tracing::error!(
                    target: "work_analyzer",
                    "run_lens_failed_state_update_failed lens_id={} worker_id={} original_error={} state_error={}",
                    lens.id,
                    worker_id,
                    err,
                    state_err
                );
            }
            tracing::error!(
                target: "work_analyzer",
                "run_lens_failed lens_id={} worker_id={} error={}",
                lens.id,
                worker_id,
                err
            );
            Err(err)
        }
        (Ok(()), Err(err)) => {
            if let Err(state_err) = lens
                .clone()
                .set_processing_state(LensProcessingState::Failed, pool)
            {
                tracing::error!(
                    target: "work_analyzer",
                    "run_lens_lock_release_failed_state_update_failed lens_id={} worker_id={} original_error={} state_error={}",
                    lens.id,
                    worker_id,
                    err,
                    state_err
                );
            }
            tracing::error!(
                target: "work_analyzer",
                "run_lens_lock_release_failed lens_id={} worker_id={} error={}",
                lens.id,
                worker_id,
                err
            );
            Err(err)
        }
        (Err(err), Err(_release_err)) => {
            if let Err(state_err) = lens
                .clone()
                .set_processing_state(LensProcessingState::Failed, pool)
            {
                tracing::error!(
                    target: "work_analyzer",
                    "run_lens_failed_and_lock_release_failed_state_update_failed lens_id={} worker_id={} original_error={} state_error={}",
                    lens.id,
                    worker_id,
                    err,
                    state_err
                );
            }
            Err(err)
        }
    }
}

async fn run_claim_loop(
    lens: &mut Lens,
    worker_id: Uuid,
    claim_cutoff_at: chrono::NaiveDateTime,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    for iteration in 0..MAX_ANALYSES_PER_RUN {
        ensure_lock_is_alive(lens, worker_id, pool)?;

        let Some(mut claimed_analysis) =
            claim_next_pending_for_lens(lens.id, claim_cutoff_at, pool)?
        else {
            tracing::info!(
                target: "work_analyzer",
                "run_lens_no_claim lens_id={} worker_id={} iteration={}",
                lens.id,
                worker_id,
                iteration
            );
            *lens = lens
                .clone()
                .set_processing_state(LensProcessingState::InSync, pool)?;
            return Ok(());
        };
        tracing::info!(
            target: "work_analyzer",
            "run_lens_claimed lens_id={} worker_id={} iteration={} analysis_id={} analysis_type={} analyzed_trace_id={:?} period_start={} period_end={}",
            lens.id,
            worker_id,
            iteration,
            claimed_analysis.id,
            claimed_analysis.landscape_analysis_type.to_db(),
            claimed_analysis.analyzed_trace_id,
            claimed_analysis.period_start,
            claimed_analysis.period_end
        );

        let claimed_analysis_for_fail = claimed_analysis.clone();
        let claimed_analysis_id = claimed_analysis.id;
        let claimed_analysis_type = claimed_analysis.landscape_analysis_type;
        let run_result = async {
            let previous_landscape_id = lens.current_landscape_id;
            if claimed_analysis.parent_analysis_id != previous_landscape_id {
                tracing::info!(
                    target: "work_analyzer",
                    "run_lens_set_parent lens_id={} analysis_id={} old_parent={:?} new_parent={:?}",
                    lens.id,
                    claimed_analysis.id,
                    claimed_analysis.parent_analysis_id,
                    previous_landscape_id
                );
                claimed_analysis.parent_analysis_id = previous_landscape_id;
                claimed_analysis = claimed_analysis.update(pool)?;
            }
            if let Some(parent_analysis_id) = previous_landscape_id {
                let copied_links = copy_landmark_links_from_analysis(
                    parent_analysis_id,
                    claimed_analysis.id,
                    pool,
                )?;
                tracing::info!(
                    target: "work_analyzer",
                    "run_lens_transfer_parent_landmarks lens_id={} analysis_id={} parent_analysis_id={} copied_links={}",
                    lens.id,
                    claimed_analysis.id,
                    parent_analysis_id,
                    copied_links
                );
            }

            run_claimed_analysis(lens, claimed_analysis.clone(), previous_landscape_id, pool).await
        }
        .await;

        if let Err(err) = run_result {
            tracing::error!(
                target: "work_analyzer",
                "run_lens_analysis_failed lens_id={} analysis_id={} analysis_type={} error={}",
                lens.id,
                claimed_analysis_id,
                claimed_analysis_type.to_db(),
                err
            );
            if let Err(state_err) = claimed_analysis_for_fail
                .set_failed(err.message.clone(), pool)
            {
                tracing::error!(
                    target: "work_analyzer",
                    "run_lens_analysis_failed_state_update_failed lens_id={} analysis_id={} analysis_type={} original_error={} state_error={}",
                    lens.id,
                    claimed_analysis_id,
                    claimed_analysis_type.to_db(),
                    err,
                    state_err
                );
            }
            return Err(err);
        }
    }

    tracing::warn!(
        target: "work_analyzer",
        "run_lens_max_iterations lens_id={} max_iterations={}",
        lens.id,
        MAX_ANALYSES_PER_RUN
    );
    Err(PpdcError::new(
        500,
        ErrorType::InternalError,
        format!(
            "Reached MAX_ANALYSES_PER_RUN ({MAX_ANALYSES_PER_RUN}) while processing lens {}",
            lens.id
        ),
    ))
}

fn ensure_lock_is_alive(lens: &Lens, worker_id: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
    let renewed = lens.renew_run_lock(worker_id, LENS_RUN_LOCK_TTL_SECONDS, pool)?;
    if renewed {
        Ok(())
    } else {
        Err(PpdcError::new(
            409,
            ErrorType::ApiError,
            format!(
                "Run lock lost while processing lens {} (worker {})",
                lens.id, worker_id
            ),
        ))
    }
}

async fn run_claimed_analysis(
    lens: &mut Lens,
    analysis: LandscapeAnalysis,
    previous_landscape_id: Option<Uuid>,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    tracing::info!(
        target: "work_analyzer",
        "run_lens_analysis_start lens_id={} analysis_id={} analysis_type={} analyzed_trace_id={:?}",
        lens.id,
        analysis.id,
        analysis.landscape_analysis_type.to_db(),
        analysis.analyzed_trace_id
    );
    let completed_analysis = if let Some(trace_id) = analysis.analyzed_trace_id {
        let trace = Trace::find_full_trace(trace_id, pool)?;
        if trace.is_encrypted {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!(
                    "Trace {} is encrypted and cannot be processed server-side",
                    trace_id
                ),
            ));
        }
        let processor = analysis_processor::AnalysisProcessor::setup(
            analysis.id,
            trace_id,
            previous_landscape_id,
            pool,
        )?;
        processor.process().await?
    } else if analysis.landscape_analysis_type == LandscapeAnalysisType::DailyRecap {
        let processor = period_analysis_processor::PeriodAnalysisProcessor::setup(
            analysis.id,
            analysis.user_id,
            previous_landscape_id,
            pool,
        )?;
        processor.process_daily_recap().await?
    } else if analysis.landscape_analysis_type == LandscapeAnalysisType::WeeklyRecap {
        let processor = period_analysis_processor::PeriodAnalysisProcessor::setup(
            analysis.id,
            analysis.user_id,
            previous_landscape_id,
            pool,
        )?;
        processor.process_weekly_recap().await?
    } else {
        analysis.set_processing_state(LandscapeProcessingState::Completed, pool)?
    };

    if let Some(trace_id) = completed_analysis.analyzed_trace_id {
        add_trace_input(
            completed_analysis.id,
            trace_id,
            LandscapeAnalysisInputType::Primary,
            pool,
        )?;
        if let Some(trace_mirror_id) = completed_analysis.trace_mirror_id {
            add_trace_mirror_input(
                completed_analysis.id,
                trace_mirror_id,
                LandscapeAnalysisInputType::Primary,
                pool,
            )?;
        }
        let trace_datetime = completed_analysis
            .interaction_date
            .unwrap_or(completed_analysis.period_end);
        let (refreshed_analyses, refreshed_traces, refreshed_trace_mirrors) =
            refresh_pending_summary_covered_inputs_for_trace(
                lens.id,
                completed_analysis.user_id,
                trace_datetime,
                pool,
            )?;
        tracing::info!(
            target: "work_analyzer",
            "run_lens_analysis_inputs_pending_summary_refresh lens_id={} analysis_id={} analysis_type={} refreshed_analyses={} traces={} trace_mirrors={} trace_datetime={}",
            lens.id,
            completed_analysis.id,
            completed_analysis.landscape_analysis_type.to_db(),
            refreshed_analyses,
            refreshed_traces,
            refreshed_trace_mirrors,
            trace_datetime
        );
    }

    *lens = lens
        .clone()
        .update_current_landscape(completed_analysis.id, pool)?;
    tracing::info!(
        target: "work_analyzer",
        "run_lens_analysis_complete lens_id={} analysis_id={} analysis_type={} new_head={}",
        lens.id,
        completed_analysis.id,
        completed_analysis.landscape_analysis_type.to_db(),
        completed_analysis.id
    );
    Ok(())
}

fn resolve_claim_cutoff_at(lens: &Lens, pool: &DbPool) -> Result<chrono::NaiveDateTime, PpdcError> {
    if let Some(target_trace_id) = lens.target_trace_id {
        let target_trace = Trace::find_full_trace(target_trace_id, pool)?;
        Ok(target_trace.interaction_date)
    } else {
        Ok(chrono::Utc::now().naive_utc())
    }
}
