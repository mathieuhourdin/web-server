use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::landmark::Landmark;
use crate::entities_v2::landscape_analysis::{
    replace_covered_inputs_for_period, LandscapeAnalysis, LandscapeProcessingState,
};

use crate::work_analyzer::active_context_filtering;
use crate::work_analyzer::analysis_context::{load_previous_landscape_inputs, AnalysisContext};
use crate::work_analyzer::mentor_feedback;
use crate::work_analyzer::period_summary;

pub struct PeriodAnalysisProcessor {
    pub context: AnalysisContext,
    pub previous_landscape: Option<LandscapeAnalysis>,
    pub previous_landscape_landmarks: Vec<Landmark>,
    pub user_high_level_projects: Vec<Landmark>,
}

impl PeriodAnalysisProcessor {
    pub fn new(
        context: AnalysisContext,
        previous_landscape: Option<LandscapeAnalysis>,
        previous_landscape_landmarks: Vec<Landmark>,
        user_high_level_projects: Vec<Landmark>,
    ) -> Self {
        Self {
            context,
            previous_landscape,
            previous_landscape_landmarks,
            user_high_level_projects,
        }
    }

    pub fn setup(
        analysis_id: Uuid,
        user_id: Uuid,
        previous_landscape_id: Option<Uuid>,
        pool: &DbPool,
    ) -> Result<Self, PpdcError> {
        let (previous_landscape, previous_landscape_landmarks, user_high_level_projects) =
            load_previous_landscape_inputs(previous_landscape_id, pool)?;
        let context = AnalysisContext {
            analysis_id,
            user_id,
            pool: pool.clone(),
        };
        Ok(Self::new(
            context,
            previous_landscape,
            previous_landscape_landmarks,
            user_high_level_projects,
        ))
    }

    pub async fn process_daily_recap(self) -> Result<LandscapeAnalysis, PpdcError> {
        let mut current_landscape =
            LandscapeAnalysis::find_full_analysis(self.context.analysis_id, &self.context.pool)?;

        if let Some(parent_analysis) = &self.previous_landscape {
            current_landscape.plain_text_state_summary =
                parent_analysis.plain_text_state_summary.clone();
            current_landscape.trace_mirror_id = parent_analysis.trace_mirror_id;
            current_landscape = current_landscape.update(&self.context.pool)?;
        }

        let lens = current_landscape
            .get_scoped_lenses(&self.context.pool)?
            .into_iter()
            .next()
            .ok_or_else(|| {
                PpdcError::new(
                    500,
                    ErrorType::InternalError,
                    format!(
                        "Daily recap analysis {} is missing a scoped lens",
                        current_landscape.id
                    ),
                )
            })?;

        let _ = replace_covered_inputs_for_period(
            current_landscape.id,
            lens.id,
            current_landscape.user_id,
            current_landscape.period_start,
            current_landscape.period_end,
            &self.context.pool,
        )?;

        let _summary = period_summary::run_day(&self.context, &current_landscape).await?;
        let _feedback = mentor_feedback::send(&self.context, &current_landscape).await?;
        let current_landscape =
            LandscapeAnalysis::find_full_analysis(self.context.analysis_id, &self.context.pool)?;
        let _linked_landmarks = active_context_filtering::run(
            &self.context,
            &current_landscape,
            &self.previous_landscape_landmarks,
            &self.user_high_level_projects,
        )?;

        let mut analysis =
            LandscapeAnalysis::find_full_analysis(self.context.analysis_id, &self.context.pool)?;
        analysis.processing_state = LandscapeProcessingState::Completed;
        analysis.update(&self.context.pool)
    }

    pub async fn process_weekly_recap(self) -> Result<LandscapeAnalysis, PpdcError> {
        let mut current_landscape =
            LandscapeAnalysis::find_full_analysis(self.context.analysis_id, &self.context.pool)?;

        if let Some(parent_analysis) = &self.previous_landscape {
            current_landscape.plain_text_state_summary =
                parent_analysis.plain_text_state_summary.clone();
            current_landscape.trace_mirror_id = parent_analysis.trace_mirror_id;
            current_landscape = current_landscape.update(&self.context.pool)?;
        }

        let lens = current_landscape
            .get_scoped_lenses(&self.context.pool)?
            .into_iter()
            .next()
            .ok_or_else(|| {
                PpdcError::new(
                    500,
                    ErrorType::InternalError,
                    format!(
                        "Weekly recap analysis {} is missing a scoped lens",
                        current_landscape.id
                    ),
                )
            })?;

        let _ = replace_covered_inputs_for_period(
            current_landscape.id,
            lens.id,
            current_landscape.user_id,
            current_landscape.period_start,
            current_landscape.period_end,
            &self.context.pool,
        )?;

        let _summary = period_summary::run_week(&self.context, &current_landscape).await?;
        let current_landscape =
            LandscapeAnalysis::find_full_analysis(self.context.analysis_id, &self.context.pool)?;
        let _linked_landmarks = active_context_filtering::run(
            &self.context,
            &current_landscape,
            &self.previous_landscape_landmarks,
            &self.user_high_level_projects,
        )?;

        let mut analysis =
            LandscapeAnalysis::find_full_analysis(self.context.analysis_id, &self.context.pool)?;
        analysis.processing_state = LandscapeProcessingState::Completed;
        analysis.update(&self.context.pool)
    }
}
