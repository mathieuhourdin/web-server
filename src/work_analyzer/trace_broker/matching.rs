use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;
use crate::work_analyzer::trace_broker::traits::{
    ExtractedElementForLandmark,
    MatchedExtractedElementForLandmark,
};
use crate::entities_v2::{
    landmark::Landmark,
};
use crate::entities::resource::resource_type::ResourceType;

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


#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "T: Serialize", deserialize = "T: DeserializeOwned"))]
pub struct LocalArrayItem<T>
{
    pub local_id: String,
    #[serde(flatten)]
    pub item: T,
}


#[derive(Serialize, Deserialize)]
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

pub fn attach_matching_results_to_elements<E, M>(
    matching_results: Vec<MatchingResult>,
    elements_local_array: LocalArray<E>,
    landmarks_local_array: LocalArray<LandmarkForMatching>
) -> Vec<M> 
where E: ExtractedElementForLandmark,
      M: MatchedExtractedElementForLandmark,
{
    let mut matched_elements: Vec<M> = vec![];

    for element in elements_local_array.items {
        let mut candidate_id = None;
        let mut confidence = 0.0;
        if let Some(matching_result) = matching_results
            .iter()
            .find(|item| item.element_id == Some(element.local_id.clone())) {
            confidence = matching_result.confidence;
            let landmark = landmarks_local_array.items
                .iter()
                .find(|item| Some(item.local_id.clone()) == matching_result.candidate_id);
            if let Some(landmark) = landmark {
                candidate_id = Some(landmark.item.id.to_string());
            } 
        }
        matched_elements.push(MatchedExtractedElementForLandmark::new(
            element.item.reference(),
            element.item.extracted_content(),
            element.item.generated_context(),
            candidate_id,
            confidence
        ));
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
    pub landmark_type: ResourceType,
}

impl From<&Landmark> for LandmarkForMatching {
    fn from(landmark: &Landmark) -> Self {
        Self { id: landmark.id.clone(), title: landmark.title.clone(), subtitle: landmark.subtitle.clone(), content: landmark.content.clone(), landmark_type: landmark.landmark_type }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::work_analyzer::trace_broker::landmark_resource_processor::{
        ExtractedElementForResource,
        MatchedExtractedElementForResource,
    };
    use uuid::Uuid;

    #[test]
    fn test_basic_matching() {
        // Un élément, un landmark, un match
        let element = ExtractedElementForResource {
            resource_label: "My Resource".to_string(),
            extracted_content: "Some content".to_string(),
            generated_context: "Some context".to_string(),
        };
        let elements = LocalArray::from_vec(vec![element]);
        
        let landmark_id = Uuid::new_v4();
        let landmark = LandmarkForMatching {
            id: landmark_id,
            title: "Landmark Title".to_string(),
            subtitle: "Subtitle".to_string(),
            content: "Content".to_string(),
            landmark_type: ResourceType::Resource,
        };
        let landmarks = LocalArray::from_vec(vec![landmark]);
        
        let matching_results = vec![MatchingResult {
            element_id: Some("0".to_string()),  // local_id est un String
            candidate_id: Some("0".to_string()),
            confidence: 0.85,
        }];
        
        let result: Vec<MatchedExtractedElementForResource> = attach_matching_results_to_elements(
            matching_results,
            elements,
            landmarks,
        );
        
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].reference(), "My Resource");
        assert_eq!(result[0].confidence(), 0.85);
        assert_eq!(result[0].landmark_id(), Some(landmark_id.to_string()));
    }
}