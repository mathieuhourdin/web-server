use chrono::{Duration, NaiveDate, NaiveDateTime};
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::analysis_summary::{AnalysisSummary, AnalysisSummaryType};
use crate::entities_v2::error::PpdcError;
use crate::entities_v2::landmark::LandmarkType;
use crate::entities_v2::landscape_analysis::model::LandscapeAnalysisType;
use crate::entities_v2::landscape_analysis::LandscapeAnalysis;
use crate::entities_v2::trace::{Trace, TraceType};
use crate::work_analyzer::analysis_context::AnalysisContext;

#[derive(Debug, Serialize)]
pub struct WeekSummaryPromptContext {
    pub period_start: NaiveDateTime,
    pub period_end: NaiveDateTime,
    pub previous_weeks_summaries: Vec<PreviousWeekSummaryContextItem>,
    pub first_week_on_platform_note: Option<String>,
    pub days: Vec<WeekDayContextItem>,
    pub days_without_traces_count: usize,
    pub high_level_projects: Vec<HighLevelProjectContextItem>,
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
pub struct HighLevelProjectContextItem {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
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
    pub user_traces: Vec<WeekDayUserTraceContextItem>,
}

#[derive(Debug, Serialize)]
pub struct WeekDayUserTraceContextItem {
    pub id: Uuid,
    pub interaction_date: NaiveDateTime,
    pub title: String,
    pub subtitle: String,
    pub content: String,
}

pub fn build(
    context: &AnalysisContext,
    analysis: &LandscapeAnalysis,
) -> Result<WeekSummaryPromptContext, PpdcError> {
    let scoped_lenses = analysis.get_scoped_lenses(&context.pool)?;
    let current_lens = scoped_lenses
        .iter()
        .find(|lens| lens.current_landscape_id == Some(analysis.id))
        .or_else(|| scoped_lenses.first());

    let (daily_analyses_by_day, previous_weeks_summaries, high_level_projects) =
        if let Some(current_lens) = current_lens {
        let all_scope_analyses = current_lens.get_analysis_scope(&context.pool)?;
        (
            build_daily_analysis_map(&all_scope_analyses, analysis),
            find_previous_weeks_summaries(analysis, &all_scope_analyses, &context.pool, 2)?,
            find_current_lens_high_level_projects(current_lens.current_landscape_id, &context.pool)?,
        )
    } else {
        (HashMap::new(), vec![], vec![])
    };
    let first_week_on_platform_note = if previous_weeks_summaries.is_empty() {
        Some("This is the first week of the user on the platform.".to_string())
    } else {
        None
    };

    let mut user_traces_by_day = find_user_traces_by_day_for_period(
        analysis.user_id,
        analysis.period_start,
        analysis.period_end,
        &context.pool,
    )?;

    let mut days = Vec::new();
    let mut days_without_traces_count = 0usize;
    let mut cursor = analysis.period_start.date();
    let end_day = analysis.period_end.date();
    while cursor < end_day {
        let day_start = cursor
            .and_hms_opt(0, 0, 0)
            .expect("valid start of day datetime");
        let day_end = day_start + Duration::days(1);
        let user_traces = user_traces_by_day.remove(&cursor).unwrap_or_default();
        let has_written_traces = !user_traces.is_empty();

        let day_context = if let Some(daily_analysis) = daily_analyses_by_day.get(&cursor) {
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
                user_traces,
            }
        } else {
            WeekDayContextItem {
                day: cursor,
                period_start: day_start,
                period_end: day_end,
                daily_analysis_id: None,
                has_daily_analysis: false,
                has_written_traces,
                no_traces_note: if has_written_traces {
                    None
                } else {
                    Some("No traces were written this day.".to_string())
                },
                summary: None,
                user_traces,
            }
        };

        if !day_context.has_written_traces {
            days_without_traces_count += 1;
        }
        days.push(day_context);
        cursor += Duration::days(1);
    }

    Ok(WeekSummaryPromptContext {
        period_start: analysis.period_start,
        period_end: analysis.period_end,
        previous_weeks_summaries,
        first_week_on_platform_note,
        days,
        days_without_traces_count,
        high_level_projects,
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

fn find_previous_weeks_summaries(
    current_analysis: &LandscapeAnalysis,
    scope_analyses: &[LandscapeAnalysis],
    pool: &DbPool,
    limit: usize,
) -> Result<Vec<PreviousWeekSummaryContextItem>, PpdcError> {
    let mut candidates = scope_analyses
        .iter()
        .filter(|analysis| analysis.landscape_analysis_type == LandscapeAnalysisType::WeeklyRecap)
        .filter(|analysis| analysis.id != current_analysis.id)
        .filter(|analysis| analysis.period_end <= current_analysis.period_start)
        .collect::<Vec<_>>();
    candidates.sort_by_key(|analysis| std::cmp::Reverse(analysis.period_end));

    let mut previous_summaries = Vec::new();
    for candidate in candidates {
        if previous_summaries.len() >= limit {
            break;
        }
        let Some(summary) = find_period_summary_for_analysis(candidate.id, pool)? else {
            continue;
        };
        previous_summaries.push(PreviousWeekSummaryContextItem {
            analysis_id: candidate.id,
            period_start: candidate.period_start,
            period_end: candidate.period_end,
            summary: SummaryContextItem {
                title: summary.title,
                short_content: summary.short_content,
                content: summary.content,
            },
        });
    }

    Ok(previous_summaries)
}

fn find_current_lens_high_level_projects(
    current_landscape_id: Option<Uuid>,
    pool: &DbPool,
) -> Result<Vec<HighLevelProjectContextItem>, PpdcError> {
    let Some(current_landscape_id) = current_landscape_id else {
        return Ok(vec![]);
    };
    let current_landscape = LandscapeAnalysis::find_full_analysis(current_landscape_id, pool)?;
    let hlps = current_landscape
        .get_landmarks(None, pool)?
        .into_iter()
        .filter(|landmark| landmark.landmark_type == LandmarkType::HighLevelProject)
        .map(|landmark| HighLevelProjectContextItem {
            id: landmark.id,
            title: landmark.title,
            subtitle: landmark.subtitle,
            content: landmark.content,
        })
        .collect::<Vec<_>>();
    Ok(hlps)
}

fn find_user_traces_by_day_for_period(
    user_id: Uuid,
    period_start: NaiveDateTime,
    period_end: NaiveDateTime,
    pool: &DbPool,
) -> Result<HashMap<NaiveDate, Vec<WeekDayUserTraceContextItem>>, PpdcError> {
    let user_traces = Trace::get_all_for_user(user_id, pool)?
        .into_iter()
        .filter(|trace| trace.trace_type == TraceType::UserTrace)
        .filter(|trace| trace.interaction_date >= period_start && trace.interaction_date < period_end)
        .collect::<Vec<_>>();

    let mut by_day: HashMap<NaiveDate, Vec<WeekDayUserTraceContextItem>> = HashMap::new();
    for trace in user_traces {
        by_day
            .entry(trace.interaction_date.date())
            .or_default()
            .push(WeekDayUserTraceContextItem {
                id: trace.id,
                interaction_date: trace.interaction_date,
                title: trace.title,
                subtitle: trace.subtitle,
                content: trace.content,
            });
    }
    for traces in by_day.values_mut() {
        traces.sort_by_key(|trace| (trace.interaction_date, trace.id));
    }
    Ok(by_day)
}
