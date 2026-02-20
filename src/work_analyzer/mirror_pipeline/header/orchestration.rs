use crate::entities::error::PpdcError;
use crate::entities_v2::{landmark::Landmark, trace::Trace, trace_mirror::TraceMirror};
use crate::work_analyzer::analysis_processor::AnalysisContext;

use super::{
    gpt_request::{build_high_level_project_index_map, extract_mirror_header},
    persistence::{create_trace_mirror, persist_high_level_project_references},
};

pub async fn run(
    trace: &Trace,
    high_level_projects: &[Landmark],
    context: &AnalysisContext,
) -> Result<TraceMirror, PpdcError> {
    let hlp_index_to_uuid = build_high_level_project_index_map(&high_level_projects);
    let header = extract_mirror_header(trace, context.analysis_id, &high_level_projects).await?;
    let trace_mirror = create_trace_mirror(trace, &header, context)?;
    persist_high_level_project_references(
        &trace_mirror,
        &header.high_level_projects,
        &hlp_index_to_uuid,
        context,
    )?;
    Ok(trace_mirror)
}
