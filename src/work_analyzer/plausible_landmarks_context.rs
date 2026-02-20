use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::PpdcError;
use crate::entities_v2::{
    element::Element,
    landmark::{Landmark, LandmarkType},
    landscape_analysis::LandscapeAnalysis,
    reference::Reference,
};

const MAX_PLAUSIBLE_LANDMARKS: usize = 50;
const MATCHING_REFERENCES_QUOTA: usize = 20;
const LAST_ELEMENT_QUOTA: usize = 10;
const ELEMENTS_COUNT_QUOTA: usize = 20;

pub fn find_ranked_plausible_landmarks_for_text(
    user_id: Uuid,
    landscape_analysis_id: Uuid,
    text: &str,
    pool: &DbPool,
    landmark_types_filter: Option<&[LandmarkType]>,
) -> Result<Vec<Landmark>, PpdcError> {
    let current_analysis = LandscapeAnalysis::find_full_analysis(landscape_analysis_id, pool)?;
    let Some(parent_analysis_id) = current_analysis.parent_analysis_id else {
        return Ok(vec![]);
    };

    let parent_analysis = LandscapeAnalysis::find_full_analysis(parent_analysis_id, pool)?;
    let parent_landmarks = parent_analysis.get_landmarks(None, pool)?;
    let mut candidate_landmarks_by_id = HashMap::new();
    for landmark in parent_landmarks {
        if landmark.landmark_type == LandmarkType::HighLevelProject {
            continue;
        }
        if let Some(filter) = landmark_types_filter {
            if !filter.contains(&landmark.landmark_type) {
                continue;
            }
        }
        candidate_landmarks_by_id.insert(landmark.id, landmark);
    }

    if candidate_landmarks_by_id.is_empty() {
        return Ok(vec![]);
    }

    let allowed_landmark_ids = candidate_landmarks_by_id
        .keys()
        .copied()
        .collect::<HashSet<_>>();
    let references = Reference::find_for_landscape_analysis(parent_analysis_id, pool)?;
    let filtered_references = references
        .into_iter()
        .filter(|reference| {
            reference.user_id == user_id
                && reference
                    .landmark_id
                    .is_some_and(|landmark_id| allowed_landmark_ids.contains(&landmark_id))
        })
        .collect::<Vec<_>>();

    let normalized_text = normalize_for_match(text);
    let mut matching_references_count = HashMap::<Uuid, usize>::new();
    for reference in &filtered_references {
        let Some(landmark_id) = reference.landmark_id else {
            continue;
        };
        if reference_matches_text(reference, &normalized_text) {
            *matching_references_count.entry(landmark_id).or_default() += 1;
        }
    }

    let mut last_related_element_at = HashMap::<Uuid, chrono::NaiveDateTime>::new();
    let mut related_elements_count = HashMap::<Uuid, usize>::new();
    for landmark_id in &allowed_landmark_ids {
        let elements = Element::find_for_landmark(*landmark_id, pool)?;
        related_elements_count.insert(*landmark_id, elements.len());

        let most_recent = elements
            .into_iter()
            .map(|element| {
                let created_at = element.created_at;
                match element.with_interaction_date(pool) {
                    Ok(hydrated) => hydrated.interaction_date.unwrap_or(hydrated.created_at),
                    Err(_) => created_at,
                }
            })
            .max();

        if let Some(date) = most_recent {
            last_related_element_at.insert(*landmark_id, date);
        }
    }

    let mut ranked_by_match = allowed_landmark_ids.iter().copied().collect::<Vec<_>>();
    ranked_by_match.sort_by_key(|id| {
        (
            Reverse(*matching_references_count.get(id).unwrap_or(&0)),
            id.to_string(),
        )
    });

    let mut ranked_by_recency = allowed_landmark_ids.iter().copied().collect::<Vec<_>>();
    ranked_by_recency.sort_by_key(|id| {
        (
            Reverse(last_related_element_at.get(id).copied()),
            id.to_string(),
        )
    });

    let mut ranked_by_elements_count = allowed_landmark_ids.iter().copied().collect::<Vec<_>>();
    ranked_by_elements_count.sort_by_key(|id| {
        (
            Reverse(*related_elements_count.get(id).unwrap_or(&0)),
            id.to_string(),
        )
    });

    let mut selected_ids = Vec::<Uuid>::new();
    let mut selected_ids_set = HashSet::<Uuid>::new();

    select_with_quota(
        &ranked_by_match,
        MATCHING_REFERENCES_QUOTA,
        &mut selected_ids,
        &mut selected_ids_set,
    );
    select_with_quota(
        &ranked_by_recency,
        LAST_ELEMENT_QUOTA,
        &mut selected_ids,
        &mut selected_ids_set,
    );
    select_with_quota(
        &ranked_by_elements_count,
        ELEMENTS_COUNT_QUOTA,
        &mut selected_ids,
        &mut selected_ids_set,
    );

    for ranked_ids in [
        &ranked_by_match,
        &ranked_by_recency,
        &ranked_by_elements_count,
    ] {
        if selected_ids.len() >= MAX_PLAUSIBLE_LANDMARKS {
            break;
        }
        for landmark_id in ranked_ids.iter().copied() {
            if selected_ids.len() >= MAX_PLAUSIBLE_LANDMARKS {
                break;
            }
            if selected_ids_set.insert(landmark_id) {
                selected_ids.push(landmark_id);
            }
        }
    }

    let selected_landmarks = selected_ids
        .into_iter()
        .filter_map(|landmark_id| candidate_landmarks_by_id.get(&landmark_id).cloned())
        .collect::<Vec<_>>();

    Ok(selected_landmarks)
}

fn reference_matches_text(reference: &Reference, normalized_text: &str) -> bool {
    if normalized_text.contains(normalize_for_match(&reference.mention).as_str()) {
        return true;
    }
    reference
        .reference_variants
        .iter()
        .any(|variant| normalized_text.contains(normalize_for_match(variant).as_str()))
}

fn normalize_for_match(value: &str) -> String {
    value.to_lowercase()
}

fn select_with_quota(
    ranked_ids: &[Uuid],
    quota: usize,
    selected_ids: &mut Vec<Uuid>,
    selected_ids_set: &mut HashSet<Uuid>,
) {
    let mut selected_for_bucket = 0;
    for landmark_id in ranked_ids.iter().copied() {
        if selected_for_bucket >= quota || selected_ids.len() >= MAX_PLAUSIBLE_LANDMARKS {
            break;
        }
        if selected_ids_set.insert(landmark_id) {
            selected_ids.push(landmark_id);
            selected_for_bucket += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities_v2::reference::ReferenceType;
    use chrono::Utc;

    fn sample_reference() -> Reference {
        let now = Utc::now().naive_utc();
        Reference {
            id: Uuid::new_v4(),
            tag_id: 1,
            trace_mirror_id: Uuid::new_v4(),
            landmark_id: Some(Uuid::new_v4()),
            landscape_analysis_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            mention: "High Output Management".to_string(),
            reference_type: ReferenceType::ProperName,
            context_tags: vec![],
            reference_variants: vec!["HOM".to_string()],
            parent_reference_id: None,
            is_user_specific: false,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn match_by_mention_is_case_insensitive() {
        let reference = sample_reference();
        let text = normalize_for_match("I re-read high output management yesterday.");
        assert!(reference_matches_text(&reference, &text));
    }

    #[test]
    fn match_by_variant_is_case_insensitive() {
        let reference = sample_reference();
        let text = normalize_for_match("I started HOM again.");
        assert!(reference_matches_text(&reference, &text));
    }

    #[test]
    fn returns_false_when_no_mention_or_variant_match() {
        let reference = sample_reference();
        let text = normalize_for_match("I worked on a coding kata.");
        assert!(!reference_matches_text(&reference, &text));
    }

    #[test]
    fn select_with_quota_deduplicates_and_respects_quota() {
        let id_1 = Uuid::new_v4();
        let id_2 = Uuid::new_v4();
        let id_3 = Uuid::new_v4();
        let ranked = vec![id_1, id_2, id_2, id_3];
        let mut selected = Vec::new();
        let mut selected_set = HashSet::new();

        select_with_quota(&ranked, 2, &mut selected, &mut selected_set);

        assert_eq!(selected, vec![id_1, id_2]);
    }

    #[test]
    fn select_with_quota_is_noop_when_quota_zero() {
        let ranked = vec![Uuid::new_v4(), Uuid::new_v4()];
        let mut selected = Vec::new();
        let mut selected_set = HashSet::new();

        select_with_quota(&ranked, 0, &mut selected, &mut selected_set);

        assert!(selected.is_empty());
    }
}
