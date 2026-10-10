use crate::entities_v2::analysis_summary::{
    AnalysisSummary, AnalysisSummaryType, MeaningfulEvent, NewAnalysisSummary,
    NewAnalysisSummaryDto, SummaryEmojis,
};
use crate::entities_v2::error::PpdcError;
use crate::work_analyzer::analysis_context::AnalysisContext;

pub fn upsert_period_recap(
    context: &AnalysisContext,
    title: String,
    short_content: String,
    content: String,
    meaningful_event: Option<MeaningfulEvent>,
    emojis: SummaryEmojis,
) -> Result<AnalysisSummary, PpdcError> {
    let existing = AnalysisSummary::find_for_analysis(context.analysis_id, &context.pool)?
        .into_iter()
        .find(|summary| summary.summary_type == AnalysisSummaryType::PeriodRecap);

    match existing {
        Some(mut summary) => {
            summary.title = title;
            summary.short_content = short_content;
            summary.content = content;
            summary.meaningful_event = meaningful_event;
            summary.activity_emoji = emojis.activity_emoji;
            summary.mood_emoji = emojis.mood_emoji;
            summary.dominant_emoji_kind = emojis.dominant_emoji_kind;
            summary.update(&context.pool)
        }
        None => NewAnalysisSummary::new(
            NewAnalysisSummaryDto {
                summary_type: Some(AnalysisSummaryType::PeriodRecap),
                title,
                short_content: Some(short_content),
                content,
                meaningful_event,
                activity_emoji: Some(emojis.activity_emoji),
                mood_emoji: Some(emojis.mood_emoji),
                dominant_emoji_kind: Some(emojis.dominant_emoji_kind),
            },
            context.analysis_id,
            context.user_id,
        )
        .create(&context.pool),
    }
}
