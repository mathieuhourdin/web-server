use crate::entities_v2::analysis_summary::AnalysisSummary;
use crate::entities_v2::error::PpdcError;
use crate::entities_v2::landscape_analysis::LandscapeAnalysis;
use crate::openai_handler::GptRequestConfig;
use crate::work_analyzer::analysis_context::AnalysisContext;

use super::day_context::build as build_day_context;
use super::persistence::upsert_period_recap;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct DaySummaryDraft {
    title: String,
    short_content: String,
    content: String,
}

pub async fn run_day(
    context: &AnalysisContext,
    analysis: &LandscapeAnalysis,
) -> Result<AnalysisSummary, PpdcError> {
    let prompt_context = build_day_context(context, analysis)?;
    let system_prompt = include_str!("day_system.md").to_string();
    let schema: serde_json::Value = serde_json::from_str(include_str!("day_schema.json"))?;
    let user_prompt = serde_json::to_string_pretty(&prompt_context)?;

    let summary = GptRequestConfig::new(
        "gpt-4.1-mini".to_string(),
        system_prompt,
        user_prompt,
        Some(schema),
        Some(context.analysis_id),
    )
    .with_display_name("Period Summary / Day Summary")
    .execute::<DaySummaryDraft>()
    .await?;

    upsert_period_recap(context, summary.title, summary.short_content, summary.content)
}
