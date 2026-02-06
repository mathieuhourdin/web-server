use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;
use crate::entities_v2::{
    landmark::{Landmark, LandmarkType},
};
use crate::entities::error::PpdcError;
use crate::openai_handler::GptRequestConfig;

#[derive(Debug, Serialize, Deserialize)]
pub struct Matches {
    pub matches: Vec<MatchingResult>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct MatchingResult {
    pub element_id: Option<String>,
    pub candidate_id: Option<String>,
    pub confidence: f32,
}

pub trait MatchableElement {
    fn id(&self) -> Uuid;
}


#[derive(Serialize, Deserialize, Debug)]
#[serde(bound(serialize = "T: Serialize", deserialize = "T: DeserializeOwned"))]
pub struct LocalArrayItem<T>
{
    pub local_id: String,
    #[serde(flatten)]
    pub item: T,
}


#[derive(Serialize, Deserialize, Debug)]
#[serde(bound(serialize = "T: Serialize", deserialize = "T: DeserializeOwned"))]
pub struct LocalArray<T> 
{
    pub items: Vec<LocalArrayItem<T>>,
}

impl<T> LocalArray<T> 
where T: Serialize + DeserializeOwned
{
    pub fn from_vec(items: Vec<T>) -> Self {
        LocalArray { items: 
        items
            .into_iter()
            .enumerate()
            .map(|(index, item)| LocalArrayItem { local_id: index.to_string(), item: item })
            .collect::<Vec<LocalArrayItem<T>>>()
        }
    }
}

pub trait ElementWithIdentifier {
    fn identifier(&self) -> String;
}

pub struct ElementMatched<ElementWithIdentifier> {
    pub element: ElementWithIdentifier,
    pub candidate_id: Option<String>,
    pub confidence: f32,
}

pub async fn match_elements<E>(
    elements: Vec<E>,
    landmarks: &Vec<Landmark>,
    system_prompt: Option<&str>,
    log_header: Option<&str>,
    analysis_id: Uuid,
    display_name: &str,
) -> Result<Vec<ElementMatched<E>>, PpdcError>
where E: ElementWithIdentifier + Clone + Serialize + DeserializeOwned,
{
    let mut result: Vec<ElementMatched<E>> = vec![];
    let mut to_process_elements = elements.clone();

    let mut matched_identifiers: std::collections::HashSet<String> = std::collections::HashSet::new();

    // First we match the elements to the landmarks by exact title match.
    for element in &elements {
        for landmark in landmarks {
            if element.identifier() == landmark.title {
                result.push(ElementMatched { element: element.clone(), candidate_id: Some(landmark.id.to_string()), confidence: 1.0 });
                matched_identifiers.insert(element.identifier());
                break;
            }
        }
    }
    to_process_elements.retain(|element| !matched_identifiers.contains(&element.identifier()));

    if to_process_elements.is_empty() {
        return Ok(result);
    }

    // Now we match the remaining elements to the landmarks using the GPT API.
    let landmarks_for_matching: Vec<LandmarkForMatching> = landmarks.iter().map(|landmark| LandmarkForMatching::from(landmark)).collect();
    let elements_local_array = LocalArray::from_vec(to_process_elements);
    let landmarks_local_array = LocalArray::from_vec(landmarks_for_matching);
    let user_prompt: String = format!("
        Elements: {}\n\n
        Candidates: {}\n\n
    ",
    serde_json::to_string(&elements_local_array.items)?,
    serde_json::to_string(&landmarks_local_array.items)?);
    let schema = include_str!("prompts/matching/schema.json").to_string();
    let system_prompt = system_prompt.unwrap_or(include_str!("prompts/matching/system.md")).to_string();
    let gpt_request_config = GptRequestConfig::new(
        "gpt-4.1-mini".to_string(),
        system_prompt.to_string(),
        &user_prompt,
        Some(serde_json::from_str(&schema).unwrap()),
        Some(analysis_id),
    )
    .with_log_header(log_header.unwrap_or("analysis_id: unknown"))
    .with_display_name(display_name);
    let matching_results: Matches = gpt_request_config.execute().await?;

    let attached_elements: Vec<ElementMatched<E>> = attach_matching_results_to_elements_with_identifier::<E>(matching_results.matches, elements_local_array, landmarks_local_array);


    result.extend(attached_elements);

    Ok(result)
}

pub fn attach_matching_results_to_elements_with_identifier<E>(
    matching_results: Vec<MatchingResult>,
    elements_local_array: LocalArray<E>,
    landmarks_local_array: LocalArray<LandmarkForMatching>
) -> Vec<ElementMatched<E>> 
where E: ElementWithIdentifier + Clone + Serialize + DeserializeOwned,
{
    let mut matched_elements: Vec<ElementMatched<E>> = vec![];
    for element in elements_local_array.items {
        let mut candidate_id = None;
        let mut confidence = 1.0;
        if let Some(matching_result) = matching_results
            .iter()
            .find(|item| item.element_id == Some(element.local_id.clone())) {
            confidence = matching_result.confidence;
            let candidate_local_id = matching_result.candidate_id.clone();
            if let Some(candidate_local_id) = candidate_local_id {
                candidate_id = landmarks_local_array.items
                    .iter()
                    .find(|item| item.local_id == candidate_local_id)
                    .map(|item| item.item.id.to_string());
                // TODO if no candidate found, there is an error in the matching_results vector.
            }
        }
        matched_elements.push(ElementMatched { element: element.item, candidate_id: candidate_id, confidence: confidence });
    }
    matched_elements
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LandmarkForMatching {
    #[serde(skip_serializing)]
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    #[serde(skip_serializing)]
    pub landmark_type: LandmarkType,
}

impl From<&Landmark> for LandmarkForMatching {
    fn from(landmark: &Landmark) -> Self {
        Self { id: landmark.id.clone(), title: landmark.title.clone(), subtitle: landmark.subtitle.clone(), content: landmark.content.clone(), landmark_type: landmark.landmark_type }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::work_analyzer::elements_pipeline::matching::LandmarkMatching;

    #[test]
    fn test_basic_matching_with_identifier() {
        // Un élément, un landmark, un match via attach_matching_results_to_elements_with_identifier
        let element = LandmarkMatching {
            temporary_id: "My Resource".to_string(),
            matching_key: "My Resource".to_string(),
            landmark_type: LandmarkType::Resource,
            candidate_id: None,
            confidence: 0.85,
        };
        let elements = LocalArray::from_vec(vec![element]);

        let landmark_id = uuid::Uuid::new_v4();
        let landmark = LandmarkForMatching {
            id: landmark_id,
            title: "Landmark Title".to_string(),
            subtitle: "Subtitle".to_string(),
            content: "Content".to_string(),
            landmark_type: LandmarkType::Resource,
        };
        let landmarks = LocalArray::from_vec(vec![landmark]);

        let matching_results = vec![MatchingResult {
            element_id: Some("0".to_string()),
            candidate_id: Some("0".to_string()),
            confidence: 0.85,
        }];

        let result: Vec<ElementMatched<LandmarkMatching>> =
            attach_matching_results_to_elements_with_identifier(
                matching_results,
                elements,
                landmarks,
            );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].element.identifier(), "My Resource");
        assert_eq!(result[0].confidence, 0.85);
        assert_eq!(result[0].candidate_id, Some(landmark_id.to_string()));
    }
}
