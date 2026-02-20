use std::collections::HashSet;

use crate::entities::error::PpdcError;
use crate::entities_v2::{
    reference::{NewReference, ReferenceType},
    trace::Trace,
    trace_mirror::{model::NewTraceMirror, TraceMirror, TraceMirrorType},
};
use crate::work_analyzer::analysis_processor::AnalysisContext;

use super::gpt_request::{MirrorHeader, MirrorHeaderTraceMirrorType, SelectedHighLevelProject};

pub fn create_trace_mirror(
    trace: &Trace,
    header: &MirrorHeader,
    context: &AnalysisContext,
) -> Result<TraceMirror, PpdcError> {
    let trace_mirror_type = match header.trace_mirror_type {
        MirrorHeaderTraceMirrorType::Bio => TraceMirrorType::Bio,
        MirrorHeaderTraceMirrorType::Journal => TraceMirrorType::Journal,
        MirrorHeaderTraceMirrorType::Note => TraceMirrorType::Note,
    };
    let trace_mirror = NewTraceMirror::new(
        header.title.clone(),
        header.subtitle.clone(),
        trace.content.clone(),
        trace_mirror_type,
        header.tags.clone(),
        trace.id,
        context.analysis_id,
        context.user_id,
        None,
        None,
        trace.interaction_date,
    );
    trace_mirror.create(&context.pool)
}

pub fn persist_high_level_project_references(
    trace_mirror: &TraceMirror,
    selected_high_level_projects: &[SelectedHighLevelProject],
    hlp_index_to_uuid: &std::collections::HashMap<i32, uuid::Uuid>,
    context: &AnalysisContext,
) -> Result<(), PpdcError> {
    let mut seen: HashSet<(i32, String)> = HashSet::new();

    for (tag_index, selection) in selected_high_level_projects.iter().enumerate() {
        let Some(landmark_id) = hlp_index_to_uuid.get(&selection.id).copied() else {
            continue;
        };

        let normalized_span = selection.span.trim().to_string();
        if normalized_span.is_empty() {
            continue;
        }

        if !seen.insert((selection.id, normalized_span.clone())) {
            continue;
        }

        let reference = NewReference::new(
            -((tag_index as i32) + 1),
            trace_mirror.id,
            Some(landmark_id),
            context.analysis_id,
            context.user_id,
            normalized_span,
            ReferenceType::PlainDesc,
            vec!["high_level_project".to_string()],
            vec![],
            None,
            false,
        );
        reference.create(&context.pool)?;
    }

    Ok(())
}
