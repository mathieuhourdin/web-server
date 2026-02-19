use std::collections::HashMap;

use uuid::Uuid;

use crate::entities::error::PpdcError;
use crate::entities::resource_relation::ResourceRelation;
use crate::entities_v2::element::Element;
use crate::entities_v2::landmark::{Landmark, LandmarkType};
use crate::entities_v2::landscape_analysis::LandscapeAnalysis;
use crate::work_analyzer::analysis_processor::AnalysisContext;
use crate::work_analyzer::elements_pipeline::creation::CreatedElements;

pub async fn refine_elements(
    context: &AnalysisContext,
    created: CreatedElements,
) -> Result<CreatedElements, PpdcError> {
    let analysis = LandscapeAnalysis::find_full_analysis(context.analysis_id, &context.pool)?;
    let analysis_elements = analysis.get_elements(&context.pool)?;
    let mut updated_by_id: HashMap<Uuid, Element> = HashMap::new();

    for element in analysis_elements {
        let linked_landmarks = find_linked_landmarks(element.id, context)?;
        let next_title = build_title_with_landmarks(&element, &linked_landmarks);
        if next_title != element.title {
            let updated_element = Element {
                title: next_title.clone(),
                subtitle: next_title,
                ..element
            }
            .update(&context.pool)?;
            updated_by_id.insert(updated_element.id, updated_element);
        }
    }

    let created_elements = created
        .created_elements
        .into_iter()
        .map(|element| updated_by_id.get(&element.id).cloned().unwrap_or(element))
        .collect::<Vec<Element>>();

    Ok(CreatedElements {
        created_landmarks: created.created_landmarks,
        created_elements,
    })
}

fn find_linked_landmarks(
    element_id: Uuid,
    context: &AnalysisContext,
) -> Result<Vec<Landmark>, PpdcError> {
    let relations = ResourceRelation::find_target_for_resource(element_id, &context.pool)?;
    let linked_landmarks = relations
        .into_iter()
        .filter(|relation| {
            relation.resource_relation.relation_type == "elmt"
                && relation.target_resource.is_landmark()
        })
        .map(|relation| Landmark::from_resource(relation.target_resource))
        .collect::<Vec<Landmark>>();
    Ok(linked_landmarks)
}

fn build_title_with_landmarks(element: &Element, landmarks: &[Landmark]) -> String {
    let (fallback_resource, fallback_author, fallback_theme) =
        parse_title_components(&element.title).unwrap_or_default();

    let mut resource_title = None;
    let mut author_title = None;
    let mut theme_title = None;

    for landmark in landmarks {
        match landmark.landmark_type {
            LandmarkType::Resource => {
                if resource_title.is_none() {
                    resource_title = Some(landmark.title.clone());
                }
            }
            LandmarkType::Person => {
                if author_title.is_none() {
                    author_title = Some(landmark.title.clone());
                }
            }
            LandmarkType::Topic => {
                if theme_title.is_none() {
                    theme_title = Some(landmark.title.clone());
                }
            }
            _ => {}
        }
    }

    let resource = resource_title.unwrap_or(fallback_resource);
    let author = author_title.unwrap_or(fallback_author);
    let theme = theme_title.unwrap_or(fallback_theme);
    let verb = if element.verb.trim().is_empty() {
        "unknown".to_string()
    } else {
        element.verb.trim().to_string()
    };

    format!("{verb}: {resource} - Par {author} Sur {theme}")
}

fn parse_title_components(title: &str) -> Option<(String, String, String)> {
    let (_, after_verb) = title.split_once(": ")?;
    let (resource, after_resource) = after_verb.split_once(" - Par ")?;
    let (author, theme) = after_resource.split_once(" Sur ")?;
    Some((resource.to_string(), author.to_string(), theme.to_string()))
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, NaiveDateTime};

    use crate::entities::resource::maturing_state::MaturingState;
    use crate::entities_v2::landmark::LandmarkType;

    use super::{build_title_with_landmarks, Element, Landmark};
    use crate::entities_v2::element::{ElementSubtype, ElementType};

    fn sample_datetime() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 1, 1)
            .expect("valid date")
            .and_hms_opt(0, 0, 0)
            .expect("valid time")
    }

    #[test]
    fn build_title_with_landmarks_uses_final_landmark_titles() {
        let element = Element {
            id: uuid::Uuid::nil(),
            title: "lire: book about management - Par Unknown Sur leadership".to_string(),
            subtitle: "".to_string(),
            content: "".to_string(),
            extended_content: None,
            element_type: ElementType::Transaction,
            element_subtype: ElementSubtype::Output,
            verb: "lire".to_string(),
            interaction_date: None,
            user_id: uuid::Uuid::nil(),
            analysis_id: uuid::Uuid::nil(),
            trace_id: uuid::Uuid::nil(),
            trace_mirror_id: None,
            landmark_id: None,
            created_at: sample_datetime(),
            updated_at: sample_datetime(),
        };

        let resource_landmark = Landmark {
            id: uuid::Uuid::new_v4(),
            title: "High Output Management".to_string(),
            subtitle: "".to_string(),
            content: "".to_string(),
            external_content_url: None,
            comment: None,
            image_url: None,
            landmark_type: LandmarkType::Resource,
            maturing_state: MaturingState::Finished,
            publishing_state: "pbsh".to_string(),
            category_id: None,
            is_external: false,
            created_at: sample_datetime(),
            updated_at: sample_datetime(),
        };
        let author_landmark = Landmark {
            id: uuid::Uuid::new_v4(),
            title: "Andrew S. Grove".to_string(),
            subtitle: "".to_string(),
            content: "".to_string(),
            external_content_url: None,
            comment: None,
            image_url: None,
            landmark_type: LandmarkType::Person,
            maturing_state: MaturingState::Finished,
            publishing_state: "pbsh".to_string(),
            category_id: None,
            is_external: false,
            created_at: sample_datetime(),
            updated_at: sample_datetime(),
        };
        let theme_landmark = Landmark {
            id: uuid::Uuid::new_v4(),
            title: "Management".to_string(),
            subtitle: "".to_string(),
            content: "".to_string(),
            external_content_url: None,
            comment: None,
            image_url: None,
            landmark_type: LandmarkType::Topic,
            maturing_state: MaturingState::Finished,
            publishing_state: "pbsh".to_string(),
            category_id: None,
            is_external: false,
            created_at: sample_datetime(),
            updated_at: sample_datetime(),
        };

        let title = build_title_with_landmarks(
            &element,
            &[resource_landmark, author_landmark, theme_landmark],
        );
        assert_eq!(
            title,
            "lire: High Output Management - Par Andrew S. Grove Sur Management"
        );
    }
}
