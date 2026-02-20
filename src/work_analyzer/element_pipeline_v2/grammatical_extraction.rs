use crate::entities::error::PpdcError;
use crate::entities_v2::{element::Element, trace_mirror::TraceMirror};
use crate::work_analyzer::analysis_processor::AnalysisContext;

use super::gpt_correction::correct_extraction;
use super::gpt_request::{request_extraction_with_prompts, GrammaticalExtractionOutput};
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
    let (
        initial_extraction,
        tag_to_landmark_id,
        hlp_id_to_uuid,
        previous_system_prompt,
        previous_user_prompt,
    ) = request_extraction_with_prompts(context, trace_mirror).await?;
    let raw_extraction = correct_extraction(
        context.analysis_id,
        previous_system_prompt,
        previous_user_prompt,
        initial_extraction,
    )
    .await?;
    let created_elements = persist_extraction(
        context,
        trace_mirror,
        &raw_extraction,
        &tag_to_landmark_id,
        &hlp_id_to_uuid,
    )?;

    Ok(GrammaticalStageOutput {
        created_elements,
        raw_extraction,
    })
}
