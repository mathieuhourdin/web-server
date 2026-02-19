use std::collections::{HashMap, HashSet};

use crate::db::DbPool;
use crate::entities::error::{ErrorType, PpdcError};
use crate::entities::resource::MaturingState;
use crate::entities_v2::landmark::{Landmark, NewLandmark};
use crate::entities_v2::landscape_analysis;
use crate::entities_v2::reference::model::NewReference;
use crate::entities_v2::trace_mirror::TraceMirror;
use uuid::Uuid;

use super::gpt_request::{
    IdentificationStatus, LandmarkReferenceContextItem, ReferencesExtractionResultItem,
};

pub fn persist_references_and_landmarks(
    analysis_id: Uuid,
    trace_mirror: &TraceMirror,
    previous_landscape_landmarks: &[Landmark],
    context: &[LandmarkReferenceContextItem],
    references: &[ReferencesExtractionResultItem],
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let groups = build_reference_groups(references);
    let previous_landmark_ids = previous_landscape_landmarks
        .iter()
        .map(|landmark| landmark.id)
        .collect::<HashSet<_>>();
    let mut mentioned_existing_landmark_ids = HashSet::new();

    for group in groups {
        let mut group_landmark_id = None;
        for index in &group {
            let reference = &references[*index];
            if reference.identification_status == IdentificationStatus::Matched {
                if let Some(local_landmark_id) = reference.landmark_id {
                    let landmark_uuid = context_landmark_uuid(local_landmark_id, context)?;
                    group_landmark_id = Some(landmark_uuid);
                    break;
                }
            }
        }

        let group_landmark_id = match group_landmark_id {
            Some(id) => id,
            None => {
                let seed_index = group
                    .iter()
                    .copied()
                    .find(|index| {
                        references[*index].identification_status == IdentificationStatus::New
                    })
                    .unwrap_or(group[0]);
                let seed_reference = &references[seed_index];
                let new_landmark = NewLandmark::new(
                    seed_reference.title_suggestion.clone(),
                    "".to_string(),
                    seed_reference.description.clone(),
                    seed_reference.landmark_type,
                    MaturingState::Draft,
                    analysis_id,
                    trace_mirror.user_id,
                    None,
                );
                new_landmark.create(pool)?.id
            }
        };

        if previous_landmark_ids.contains(&group_landmark_id) {
            mentioned_existing_landmark_ids.insert(group_landmark_id);
        }

        for index in group {
            let reference = &references[index];
            let new_reference = NewReference::new(
                reference.tag_id,
                trace_mirror.id,
                Some(group_landmark_id),
                analysis_id,
                trace_mirror.user_id,
                reference.mention.clone(),
                reference.reference_type,
                reference.context_tags.clone(),
                reference.reference_variants.clone(),
                None,
                false,
            );
            let _new_reference = new_reference.create(pool)?;
        }
    }

    for landmark in previous_landscape_landmarks {
        if !mentioned_existing_landmark_ids.contains(&landmark.id) {
            landscape_analysis::add_landmark_ref(
                analysis_id,
                landmark.id,
                trace_mirror.user_id,
                pool,
            )?;
        }
    }

    Ok(())
}

fn context_landmark_uuid(
    local_landmark_id: i32,
    context: &[LandmarkReferenceContextItem],
) -> Result<Uuid, PpdcError> {
    context
        .iter()
        .find(|item| item.landmark_id == local_landmark_id)
        .map(|item| item.uuid)
        .ok_or_else(|| {
            PpdcError::new(
                400,
                ErrorType::ApiError,
                format!(
                    "Matched reference landmark_id {} not found in context",
                    local_landmark_id
                ),
            )
        })
}

fn build_reference_groups(references: &[ReferencesExtractionResultItem]) -> Vec<Vec<usize>> {
    let mut tag_to_index = HashMap::new();
    for (index, reference) in references.iter().enumerate() {
        tag_to_index.insert(reference.tag_id, index);
    }

    let mut adjacency = vec![Vec::<usize>::new(); references.len()];
    for (index, reference) in references.iter().enumerate() {
        if let Some(other_tag_id) = reference.same_object_tag_id {
            if let Some(&other_index) = tag_to_index.get(&other_tag_id) {
                if other_index != index {
                    adjacency[index].push(other_index);
                    adjacency[other_index].push(index);
                }
            }
        }
    }

    let mut visited = vec![false; references.len()];
    let mut groups = Vec::new();
    for index in 0..references.len() {
        if visited[index] {
            continue;
        }
        let mut stack = vec![index];
        visited[index] = true;
        let mut group = Vec::new();
        while let Some(current) = stack.pop() {
            group.push(current);
            for &neighbor in &adjacency[current] {
                if !visited[neighbor] {
                    visited[neighbor] = true;
                    stack.push(neighbor);
                }
            }
        }
        groups.push(group);
    }
    groups
}
