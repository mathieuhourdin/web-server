use chrono::{Duration, NaiveDate, NaiveDateTime};
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::analysis_summary::{AnalysisSummary, AnalysisSummaryType};
use crate::entities_v2::error::PpdcError;
use crate::entities_v2::landscape_analysis::model::{LandscapeAnalysisInputType, LandscapeAnalysisType};
use crate::entities_v2::landscape_analysis::LandscapeAnalysis;
use crate::work_analyzer::analysis_context::AnalysisContext;

#[derive(Debug, Serialize)]
pub struct WeekSummaryPromptContext {
    pub analysis_id: Uuid,
    pub user_id: Uuid,
    pub analysis_type: String,
    pub period_start: NaiveDateTime,
    pub period_end: NaiveDateTime,
    pub current_state_summary: String,
    pub existing_period_summary: Option<SummaryContextItem>,
    pub previous_week_summary: Option<PreviousWeekSummaryContextItem>,
    pub days: Vec<WeekDayContextItem>,
    pub days_without_traces_count: usize,
}

#[derive(Debug, Serialize)]
pub struct SummaryContextItem {
    pub title: String,
    pub short_content: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct PreviousWeekSummaryContextItem {
    pub analysis_id: Uuid,
    pub period_start: NaiveDateTime,
    pub period_end: NaiveDateTime,
    pub summary: SummaryContextItem,
}

#[derive(Debug, Serialize)]
pub struct WeekDayContextItem {
    pub day: NaiveDate,
    pub period_start: NaiveDateTime,
    pub period_end: NaiveDateTime,
    pub daily_analysis_id: Option<Uuid>,
    pub has_daily_analysis: bool,
    pub has_written_traces: bool,
    pub no_traces_note: Option<String>,
    pub summary: Option<SummaryContextItem>,
}

pub fn build(
    context: &AnalysisContext,
    analysis: &LandscapeAnalysis,
) -> Result<WeekSummaryPromptContext, PpdcError> {
    let existing_period_summary = find_period_summary_for_analysis(analysis.id, &context.pool)?
        .map(|summary| SummaryContextItem {
            title: summary.title,
            short_content: summary.short_content,
            content: summary.content,
        });

    let scoped_lenses = analysis.get_scoped_lenses(&context.pool)?;
    let current_lens = scoped_lenses
        .iter()
        .find(|lens| lens.current_landscape_id == Some(analysis.id))
        .or_else(|| scoped_lenses.first());

    let (daily_analyses_by_day, previous_week_summary) = if let Some(current_lens) = current_lens {
        let all_scope_analyses = current_lens.get_analysis_scope(&context.pool)?;
        (
            build_daily_analysis_map(&all_scope_analyses, analysis),
            find_previous_week_summary(analysis, &all_scope_analyses, &context.pool)?,
        )
    } else {
        (HashMap::new(), None)
    };

    let mut days = Vec::new();
    let mut days_without_traces_count = 0usize;
    let mut cursor = analysis.period_start.date();
    let end_day = analysis.period_end.date();
    while cursor <= end_day {
        let day_start = cursor
            .and_hms_opt(0, 0, 0)
            .expect("valid start of day datetime");
        let day_end = cursor
            .and_hms_opt(23, 59, 59)
            .expect("valid end of day datetime");

        let day_context = if let Some(daily_analysis) = daily_analyses_by_day.get(&cursor) {
            let has_written_traces = daily_analysis
                .get_inputs(&context.pool)?
                .into_iter()
                .any(|input| {
                    input.input_type == LandscapeAnalysisInputType::Covered && input.trace_id.is_some()
                });
            let summary = find_period_summary_for_analysis(daily_analysis.id, &context.pool)?
                .map(|summary| SummaryContextItem {
                    title: summary.title,
                    short_content: summary.short_content,
                    content: summary.content,
                });
            let no_traces_note = if has_written_traces {
                None
            } else {
                Some("No traces were written this day.".to_string())
            };

            WeekDayContextItem {
                day: cursor,
                period_start: daily_analysis.period_start,
                period_end: daily_analysis.period_end,
                daily_analysis_id: Some(daily_analysis.id),
                has_daily_analysis: true,
                has_written_traces,
                no_traces_note,
                summary,
            }
        } else {
            WeekDayContextItem {
                day: cursor,
                period_start: day_start,
                period_end: day_end,
                daily_analysis_id: None,
                has_daily_analysis: false,
                has_written_traces: false,
                no_traces_note: Some("No traces were written this day.".to_string()),
                summary: None,
            }
        };

        if !day_context.has_written_traces {
            days_without_traces_count += 1;
        }
        days.push(day_context);
        cursor += Duration::days(1);
    }

    Ok(WeekSummaryPromptContext {
        analysis_id: analysis.id,
        user_id: analysis.user_id,
        analysis_type: analysis.landscape_analysis_type.to_db().to_string(),
        period_start: analysis.period_start,
        period_end: analysis.period_end,
        current_state_summary: analysis.plain_text_state_summary.clone(),
        existing_period_summary,
        previous_week_summary,
        days,
        days_without_traces_count,
    })
}

fn build_daily_analysis_map(
    scope_analyses: &[LandscapeAnalysis],
    current_week_analysis: &LandscapeAnalysis,
) -> HashMap<NaiveDate, LandscapeAnalysis> {
    let mut by_day = HashMap::new();
    for analysis in scope_analyses
        .iter()
        .filter(|analysis| analysis.landscape_analysis_type == LandscapeAnalysisType::DailyRecap)
        .filter(|analysis| analysis.period_start >= current_week_analysis.period_start)
        .filter(|analysis| analysis.period_end <= current_week_analysis.period_end)
    {
        let day = analysis.period_start.date();
        let should_replace = by_day
            .get(&day)
            .map(|existing: &LandscapeAnalysis| existing.updated_at < analysis.updated_at)
            .unwrap_or(true);
        if should_replace {
            by_day.insert(day, analysis.clone());
        }
    }
    by_day
}

fn find_period_summary_for_analysis(
    analysis_id: Uuid,
    pool: &DbPool,
) -> Result<Option<AnalysisSummary>, PpdcError> {
    Ok(AnalysisSummary::find_for_analysis(analysis_id, pool)?
        .into_iter()
        .find(|summary| summary.summary_type == AnalysisSummaryType::PeriodRecap))
}

fn find_previous_week_summary(
    current_analysis: &LandscapeAnalysis,
    scope_analyses: &[LandscapeAnalysis],
    pool: &DbPool,
) -> Result<Option<PreviousWeekSummaryContextItem>, PpdcError> {
    let previous_week_analysis = scope_analyses
        .iter()
        .filter(|analysis| analysis.landscape_analysis_type == LandscapeAnalysisType::WeeklyRecap)
        .filter(|analysis| analysis.id != current_analysis.id)
        .filter(|analysis| analysis.period_end <= current_analysis.period_start)
        .max_by_key(|analysis| analysis.period_end);

    let Some(previous_week_analysis) = previous_week_analysis else {
        return Ok(None);
    };
    let Some(summary) = find_period_summary_for_analysis(previous_week_analysis.id, pool)? else {
        return Ok(None);
    };

    Ok(Some(PreviousWeekSummaryContextItem {
        analysis_id: previous_week_analysis.id,
        period_start: previous_week_analysis.period_start,
        period_end: previous_week_analysis.period_end,
        summary: SummaryContextItem {
            title: summary.title,
            short_content: summary.short_content,
            content: summary.content,
        },
    }))
}
