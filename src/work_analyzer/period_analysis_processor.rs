use chrono::{Duration, LocalResult, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::analysis_summary::{AnalysisSummary, AnalysisSummaryType};
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::landmark::Landmark;
use crate::entities_v2::landscape_analysis::{
    replace_covered_inputs_for_period, LandscapeAnalysis, LandscapeProcessingState,
};
use crate::entities_v2::message::Message;
use crate::entities_v2::platform_infra::mailer::{self, NewOutboundEmail, OutboundEmailProvider};
use crate::entities_v2::user::User;
use crate::environment;

use crate::work_analyzer::active_context_filtering;
use crate::work_analyzer::analysis_context::{load_previous_landscape_inputs, AnalysisContext};
use crate::work_analyzer::mentor_feedback;
use crate::work_analyzer::period_summary;

fn parse_user_timezone_or_utc(user: &User) -> Tz {
    user.timezone.parse::<Tz>().unwrap_or(chrono_tz::UTC)
}

fn resolve_local_time(tz: Tz, local_naive: chrono::NaiveDateTime) -> chrono::DateTime<Tz> {
    match tz.from_local_datetime(&local_naive) {
        LocalResult::Single(dt) => dt,
        LocalResult::Ambiguous(early, _) => early,
        LocalResult::None => {
            let mut candidate = local_naive + Duration::hours(1);
            loop {
                match tz.from_local_datetime(&candidate) {
                    LocalResult::Single(dt) => break dt,
                    LocalResult::Ambiguous(early, _) => break early,
                    LocalResult::None => {
                        candidate += Duration::hours(1);
                    }
                }
            }
        }
    }
}

fn next_local_nine_am_utc(user: &User) -> chrono::NaiveDateTime {
    let tz = parse_user_timezone_or_utc(user);
    let now_local = Utc::now().with_timezone(&tz);
    let today_nine_local_naive = now_local.date_naive().and_time(
        NaiveTime::from_hms_opt(9, 0, 0).expect("09:00:00 should always be a valid local time"),
    );
    let scheduled_local = if now_local.naive_local() < today_nine_local_naive {
        resolve_local_time(tz, today_nine_local_naive)
    } else {
        resolve_local_time(tz, today_nine_local_naive + Duration::days(1))
    };
    scheduled_local.with_timezone(&Utc).naive_utc()
}

fn build_preview(content: &str, max_chars: usize) -> String {
    let trimmed = content.trim();
    let preview = trimmed.chars().take(max_chars).collect::<String>();
    if trimmed.chars().count() > max_chars {
        format!("{}...", preview)
    } else {
        preview
    }
}

fn schedule_daily_recap_email(
    analysis: &LandscapeAnalysis,
    pool: &DbPool,
) -> Result<Option<(Uuid, chrono::NaiveDateTime)>, PpdcError> {
    let user = User::find(&analysis.user_id, pool)?;
    if user.email.trim().is_empty() {
        return Ok(None);
    }

    let recap_summary = AnalysisSummary::find_for_analysis(analysis.id, pool)?
        .into_iter()
        .find(|summary| summary.summary_type == AnalysisSummaryType::PeriodRecap)
        .ok_or_else(|| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Daily recap analysis {} is missing its period summary", analysis.id),
            )
        })?;

    let Some(feedback_message) = Message::find_latest_feedback_for_analysis(
        analysis.id,
        analysis.user_id,
        pool,
    )? else {
        return Ok(None);
    };
    let mentor = User::find(&feedback_message.sender_user_id, pool)?;

    let recap_url = format!(
        "{}/me/home?feedback=open",
        environment::get_app_base_url().trim_end_matches('/')
    );
    let template = mailer::daily_recap_email(
        &user.display_name(),
        &mentor.display_name(),
        &feedback_message.title,
        &build_preview(&feedback_message.content, 220),
        &recap_summary.title,
        &build_preview(&recap_summary.short_content, 180),
        &recap_url,
    );
    let scheduled_at = next_local_nine_am_utc(&user);
    let email = NewOutboundEmail::new(
        Some(user.id),
        "DAILY_RECAP_EMAIL".to_string(),
        Some("LANDSCAPE_ANALYSIS".to_string()),
        Some(analysis.id),
        user.email.clone(),
        "Matiere Grise <noreply@ppdcoeur.fr>".to_string(),
        template.subject,
        template.text_body,
        template.html_body,
        OutboundEmailProvider::Resend,
        Some(scheduled_at),
    )
    .create(pool)?;

    Ok(Some((email.id, scheduled_at)))
}

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
        let analysis = analysis.update(&self.context.pool)?;

        match schedule_daily_recap_email(&analysis, &self.context.pool) {
            Ok(Some((email_id, scheduled_at))) => {
                tracing::info!(
                    target: "mailer",
                    "daily_recap_email_scheduled analysis_id={} user_id={} email_id={} scheduled_at={}",
                    analysis.id,
                    analysis.user_id,
                    email_id,
                    scheduled_at
                );
            }
            Ok(None) => {
                tracing::info!(
                    target: "mailer",
                    "daily_recap_email_skipped analysis_id={} user_id={} reason=no_recipient_email_or_feedback",
                    analysis.id,
                    analysis.user_id
                );
            }
            Err(err) => {
                tracing::error!(
                    target: "mailer",
                    "daily_recap_email_schedule_failed analysis_id={} user_id={} error={}",
                    analysis.id,
                    analysis.user_id,
                    err
                );
            }
        }

        Ok(analysis)
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
