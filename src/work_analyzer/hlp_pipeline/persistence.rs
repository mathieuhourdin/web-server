use std::collections::HashSet;

use diesel::prelude::*;

use crate::entities::error::PpdcError;
use crate::entities::resource::MaturingState;
use crate::schema::landmark_relations;
use crate::entities_v2::{
    landmark::{Landmark, LandmarkType, NewLandmark},
    reference::{NewReference, ReferenceType},
    trace::Trace,
    trace_mirror::{NewTraceMirror, TraceMirror, TraceMirrorType},
};
use crate::work_analyzer::analysis_processor::AnalysisContext;

use super::gpt_request::{HighLevelProjectDraft, RelatedLandmarkDraft};

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

            let mut conn = context
                .pool
                .get()
                .expect("Failed to get a connection from the pool");
            diesel::insert_into(landmark_relations::table)
                .values((
                    landmark_relations::origin_landmark_id.eq(landmark.id),
                    landmark_relations::target_landmark_id.eq(project_landmark_id),
                    landmark_relations::relation_type.eq("HIGH_LEVEL_PROJECT_RELATED_TO"),
                ))
                .on_conflict((
                    landmark_relations::origin_landmark_id,
                    landmark_relations::target_landmark_id,
                    landmark_relations::relation_type,
                ))
                .do_nothing()
                .execute(&mut conn)?;
        }

        created_related_landmarks.push(landmark);
    }

    Ok((trace_mirror, created_projects, created_related_landmarks))
}
