use crate::entities::error::PpdcError;
use crate::entities_v2::{landmark::Landmark, trace::Trace, trace_mirror::TraceMirror};
use crate::work_analyzer::analysis_processor::AnalysisContext;

use super::{
    gpt_request::extract_high_level_projects,
    persistence::persist_hlp_entities,
};

#[derive(Debug, Clone)]
pub struct HlpPipelineOutput {
    pub trace_mirror: TraceMirror,
    pub created_high_level_projects: Vec<Landmark>,
    pub created_related_landmarks: Vec<Landmark>,
}

pub async fn run(context: &AnalysisContext, trace: &Trace) -> Result<HlpPipelineOutput, PpdcError> {
    let extraction = extract_high_level_projects(&trace.content, context.analysis_id).await?;
    let (trace_mirror, created_high_level_projects, created_related_landmarks) =
        persist_hlp_entities(context, trace, &extraction.projects, &extraction.landmarks)?;

    Ok(HlpPipelineOutput {
        trace_mirror,
        created_high_level_projects,
        created_related_landmarks,
    })
}
