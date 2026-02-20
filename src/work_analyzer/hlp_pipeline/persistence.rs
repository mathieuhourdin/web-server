use std::collections::HashSet;

use crate::entities::error::PpdcError;
use crate::entities::resource::MaturingState;
use crate::entities::resource_relation::{NewResourceRelation, ResourceRelation};
use crate::entities_v2::{
    landmark::{Landmark, LandmarkType, NewLandmark},
    reference::{NewReference, ReferenceType},
    trace::Trace,
    trace_mirror::{NewTraceMirror, TraceMirror, TraceMirrorType},
};
use crate::work_analyzer::analysis_processor::AnalysisContext;

use super::gpt_request::{HighLevelProjectDraft, RelatedLandmarkDraft};

const HLP_LANDMARK_RELATION_TYPE: &str = "hlpr";

pub fn persist_hlp_entities(
    context: &AnalysisContext,
    trace: &Trace,
    project_drafts: &[HighLevelProjectDraft],
    related_landmark_drafts: &[RelatedLandmarkDraft],
) -> Result<(TraceMirror, Vec<Landmark>, Vec<Landmark>), PpdcError> {
    let trace_mirror = NewTraceMirror::new(
        "High Level Project Definition".to_string(),
        "".to_string(),
        trace.content.clone(),
        TraceMirrorType::HighLevelProjects,
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
    let mut created_related_landmarks = Vec::new();
    let mut project_landmark_by_id = std::collections::HashMap::new();
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
        ensure_landmark_ownr_link(landmark.id, context.analysis_id, context.user_id, &context.pool)?;

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

        project_landmark_by_id.insert(draft.id, landmark.id);
        created_projects.push(landmark);
    }

    for draft in related_landmark_drafts {
        if draft.landmark_type == LandmarkType::HighLevelProject {
            continue;
        }

        let new_landmark = NewLandmark::new(
            draft.title.clone(),
            draft.subtitle.clone(),
            draft.content.clone(),
            draft.landmark_type,
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
                vec!["high_level_project_related".to_string()],
                vec![],
                None,
                false,
            );
            reference.create(&context.pool)?;
            tag_id_counter -= 1;
        }

        let mut seen_related_projects = HashSet::new();
        for project_id in &draft.related_project_ids {
            if !seen_related_projects.insert(*project_id) {
                continue;
            }
            let Some(project_landmark_id) = project_landmark_by_id.get(project_id).copied() else {
                continue;
            };

            let mut relation = NewResourceRelation::new(landmark.id, project_landmark_id);
            relation.relation_type = Some(HLP_LANDMARK_RELATION_TYPE.to_string());
            relation.user_id = Some(context.user_id);
            relation.create(&context.pool)?;
        }

        created_related_landmarks.push(landmark);
    }

    Ok((trace_mirror, created_projects, created_related_landmarks))
}

fn ensure_landmark_ownr_link(
    landmark_id: uuid::Uuid,
    analysis_id: uuid::Uuid,
    user_id: uuid::Uuid,
    pool: &crate::db::DbPool,
) -> Result<(), PpdcError> {
    let already_linked = ResourceRelation::find_target_for_resource(landmark_id, pool)?
        .into_iter()
        .any(|relation| {
            relation.resource_relation.relation_type == "ownr"
                && relation.target_resource.id == analysis_id
        });

    if already_linked {
        return Ok(());
    }

    let mut relation = NewResourceRelation::new(landmark_id, analysis_id);
    relation.relation_type = Some("ownr".to_string());
    relation.user_id = Some(user_id);
    relation.create(pool)?;
    Ok(())
}
