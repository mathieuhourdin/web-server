use crate::entities_v2::analysis_summary::{
    AnalysisSummary, AnalysisSummaryType, MeaningfulEvent, NewAnalysisSummary,
    NewAnalysisSummaryDto,
};
use crate::entities_v2::error::PpdcError;
use crate::work_analyzer::analysis_context::AnalysisContext;

pub fn upsert_period_recap(
    context: &AnalysisContext,
    title: String,
    short_content: String,
    content: String,
    meaningful_event: Option<MeaningfulEvent>,
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
            summary.update(&context.pool)
        }
        None => NewAnalysisSummary::new(
            NewAnalysisSummaryDto {
                summary_type: Some(AnalysisSummaryType::PeriodRecap),
                title,
                short_content: Some(short_content),
                content,
                meaningful_event,
            },
            context.analysis_id,
            context.user_id,
        )
        .create(&context.pool),
    }
}
