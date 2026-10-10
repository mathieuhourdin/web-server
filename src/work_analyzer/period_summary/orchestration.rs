use crate::entities_v2::analysis_summary::AnalysisSummary;
use crate::entities_v2::analysis_summary::MeaningfulEvent;
use crate::entities_v2::analysis_summary::SummaryEmojis;
use crate::entities_v2::error::PpdcError;
use crate::entities_v2::landscape_analysis::LandscapeAnalysis;
use crate::openai_handler::GptRequestConfig;
use crate::work_analyzer::analysis_context::AnalysisContext;

use super::day_context::build as build_day_context;
use super::persistence::upsert_period_recap;
use super::week_context::build as build_week_context;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PeriodSummaryDraft {
    title: String,
    short_content: String,
    content: String,
    meaningful_event: Option<MeaningfulEvent>,
    #[serde(flatten)]
    emojis: SummaryEmojis,
}

pub async fn run_day(
    context: &AnalysisContext,
    analysis: &LandscapeAnalysis,
) -> Result<AnalysisSummary, PpdcError> {
    let prompt_context = build_day_context(context, analysis)?;
    let system_prompt = format!(
        "{}\n\n{}",
        include_str!("day_system.md"),
        include_str!("emojis_system.md")
    );
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
    .execute::<PeriodSummaryDraft>()
    .await?;

    upsert_period_recap(
        context,
        summary.title,
        summary.short_content,
        summary.content,
        summary.meaningful_event,
        summary.emojis,
    )
}

pub async fn run_week(
    context: &AnalysisContext,
    analysis: &LandscapeAnalysis,
) -> Result<AnalysisSummary, PpdcError> {
    let prompt_context = build_week_context(context, analysis)?;
    let system_prompt = format!(
        "{}\n\n{}",
        include_str!("week_system.md"),
        include_str!("emojis_system.md")
    );
    let schema: serde_json::Value = serde_json::from_str(include_str!("week_schema.json"))?;
    let user_prompt = serde_json::to_string_pretty(&prompt_context)?;

    let summary = GptRequestConfig::new(
        "gpt-4.1-mini".to_string(),
        system_prompt,
        user_prompt,
        Some(schema),
        Some(context.analysis_id),
    )
    .with_display_name("Period Summary / Week Summary")
    .execute::<PeriodSummaryDraft>()
    .await?;

    upsert_period_recap(
        context,
        summary.title,
        summary.short_content,
        summary.content,
        summary.meaningful_event,
        summary.emojis,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities_v2::analysis_summary::SummaryEmojiKind;

    #[test]
    fn daily_and_weekly_schemas_request_nullable_cues_and_a_selector() {
        for raw in [
            include_str!("day_schema.json"),
            include_str!("week_schema.json"),
        ] {
            let schema: serde_json::Value = serde_json::from_str(raw).unwrap();
            for field in ["activity_emoji", "mood_emoji", "dominant_emoji_kind"] {
                assert!(schema["required"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!(field)));
                assert_eq!(
                    schema["properties"][field]["type"],
                    serde_json::json!(["string", "null"])
                );
            }
            assert_eq!(
                schema["properties"]["dominant_emoji_kind"]["enum"],
                serde_json::json!(["activity", "mood", null])
            );
        }
    }

    #[test]
    fn generation_draft_reads_nullable_and_dominant_emoji_fields() {
        let draft: PeriodSummaryDraft = serde_json::from_value(serde_json::json!({
            "title":"Trip", "short_content":"A trip", "content":"A restful trip",
            "activity_emoji":"🚆", "mood_emoji":"😌", "dominant_emoji_kind":"mood",
            "meaningful_event":null
        }))
        .unwrap();
        assert_eq!(
            draft.emojis.dominant_emoji_kind,
            Some(SummaryEmojiKind::Mood)
        );
        assert_eq!(
            draft.emojis.normalized().display_emoji().as_deref(),
            Some("😌")
        );
        let old: PeriodSummaryDraft = serde_json::from_value(serde_json::json!({
            "title":"Day", "short_content":"Short", "content":"Recap"
        }))
        .unwrap();
        assert_eq!(old.emojis.normalized().display_emoji(), None);
    }
}
