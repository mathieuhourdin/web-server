use std::collections::HashSet;

use crate::entities::error::PpdcError;
use crate::entities::resource::MaturingState;
use crate::entities_v2::{
    landmark::{Landmark, LandmarkType, NewLandmark},
    reference::{NewReference, ReferenceType},
    trace::Trace,
    trace_mirror::{NewTraceMirror, TraceMirror},
};
use crate::work_analyzer::analysis_processor::AnalysisContext;

use super::gpt_request::HighLevelProjectDraft;

pub fn persist_hlp_entities(
    context: &AnalysisContext,
    trace: &Trace,
    project_drafts: &[HighLevelProjectDraft],
) -> Result<(TraceMirror, Vec<Landmark>), PpdcError> {
    let trace_mirror = NewTraceMirror::new(
        "High Level Project Definition".to_string(),
        "".to_string(),
        trace.content.clone(),
        Vec::new(),
        trace.id,
        context.analysis_id,
        context.user_id,
        None,
        None,
        trace.interaction_date,
    )
    .create(&context.pool)?;

    let mut created_projects = Vec::new();
    let mut tag_id_counter: i32 = -1;

    for draft in project_drafts {
        let new_landmark = NewLandmark::new(
            draft.title.clone(),
            draft.subtitle.clone(),
            draft.content.clone(),
            LandmarkType::HighLevelProject,
            MaturingState::Draft,
            context.analysis_id,
            context.user_id,
            None,
        );
        let landmark = new_landmark.create(&context.pool)?;

        let mut mentions: Vec<String> = draft
            .spans
            .iter()
            .map(|span| span.trim().to_string())
            .filter(|span| !span.is_empty())
            .collect();
        if mentions.is_empty() {
            mentions.push(draft.title.clone());
        }

        let mut seen_mentions = HashSet::new();
        for mention in mentions {
            if !seen_mentions.insert(mention.clone()) {
                continue;
            }

            let reference = NewReference::new(
                tag_id_counter,
                trace_mirror.id,
                Some(landmark.id),
                context.analysis_id,
                context.user_id,
                mention,
                ReferenceType::PlainDesc,
                vec!["high_level_project".to_string()],
                vec![],
                None,
                false,
            );
            reference.create(&context.pool)?;
            tag_id_counter -= 1;
        }

        created_projects.push(landmark);
    }

    Ok((trace_mirror, created_projects))
}
