use crate::entities::error::PpdcError;
use crate::entities_v2::{element::Element, trace_mirror::TraceMirror};
use crate::work_analyzer::analysis_processor::AnalysisContext;

use super::gpt_request::{request_extraction, GrammaticalExtractionOutput};
use super::persistence::persist_extraction;

#[derive(Debug, Clone)]
pub struct GrammaticalStageOutput {
    pub created_elements: Vec<Element>,
    pub raw_extraction: GrammaticalExtractionOutput,
}

pub async fn run(
    context: &AnalysisContext,
    trace_mirror: &TraceMirror,
) -> Result<GrammaticalStageOutput, PpdcError> {
    let (raw_extraction, tag_to_landmark_id) = request_extraction(context, trace_mirror).await?;
    let created_elements =
        persist_extraction(context, trace_mirror, &raw_extraction, &tag_to_landmark_id)?;

    Ok(GrammaticalStageOutput {
        created_elements,
        raw_extraction,
    })
}
