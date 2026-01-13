use crate::entities::error::PpdcError;
use crate::entities::resource::{NewResource, Resource, maturing_state::MaturingState};
use crate::entities::resource_relation::NewResourceRelation;
use crate::entities::resource::resource_type::ResourceType;
use crate::entities::landmark::{Landmark, NewLandmark, landmark_create_child_and_return, create_landmark_for_analysis};
use crate::openai_handler::GptRequestConfig;
use crate::work_analyzer::trace_broker::traits::ProcessorConfig;
use crate::work_analyzer::trace_broker::{
    types::{
        NewLandmarkForExtractedElementType, MatchedExtractedElementForLandmarkType
    },
    traits::{
        ProcessorContext,
        LandmarkProcessor,
        ExtractedElementForLandmark, MatchedExtractedElementForLandmark, NewLandmarkForExtractedElement
    }
};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use crate::db::DbPool;
use async_trait::async_trait;

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedElementForResource {
    pub resource: String,
    pub extracted_content: String,
    pub generated_context: String,
}

impl ExtractedElementForLandmark for ExtractedElementForResource {
    fn reference(&self) -> String {
        self.resource.clone()
    }
    fn extracted_content(&self) -> String {
        self.extracted_content.clone()
    }
    fn generated_context(&self) -> String {
        self.generated_context.clone()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatchedExtractedElementForResource {
    pub resource: String,
    pub extracted_content: String,
    pub generated_context: String,
    pub resource_id: Option<String>,
}

impl MatchedExtractedElementForLandmark for MatchedExtractedElementForResource {
    fn title(&self) -> String {
        self.resource.clone()
    }
    fn subtitle(&self) -> String {
        "".to_string()
    }
    fn extracted_content(&self) -> String {
        self.extracted_content.clone()
    }
    fn generated_context(&self) -> String {
        self.generated_context.clone()
    }
    fn landmark_id(&self) -> Option<String> {
        self.resource_id.clone()
    }
    fn landmark_type(&self) -> ResourceType {
        ResourceType::Resource
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct MatchedElementsForResources {
    pub matched_elements: Vec<MatchedExtractedElementForResource>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedElementsForResources {
    pub elements: Vec<ExtractedElementForResource>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewResourceForExtractedElement {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub author: String,
    pub identified: bool,
    pub resource_type: String,
}

impl NewLandmarkForExtractedElement for NewResourceForExtractedElement {
    fn title(&self) -> String {
        self.title.clone()
    }
    fn subtitle(&self) -> String {
        self.subtitle.clone()
    }
    fn content(&self) -> String {
        self.content.clone()
    }
    fn identified(&self) -> bool {
        self.identified
    }
    fn landmark_type(&self) -> ResourceType {
        ResourceType::Resource
    }
}
impl From<NewResourceForExtractedElement> for NewLandmarkForExtractedElementType {
    fn from(new_resource_for_extracted_element: NewResourceForExtractedElement) -> Self {
        Self {
            title: new_resource_for_extracted_element.title,
            subtitle: format!("{} par {} - {}", new_resource_for_extracted_element.resource_type, new_resource_for_extracted_element.author, new_resource_for_extracted_element.subtitle),
            content: new_resource_for_extracted_element.content,
            identified: new_resource_for_extracted_element.identified,
            landmark_type: ResourceType::Resource
        }
    }
}

impl From<NewLandmarkForExtractedElementType> for NewLandmark {
    fn from(new_landmark_for_extracted_element: NewLandmarkForExtractedElementType) -> Self {
        NewLandmark::new(
            new_landmark_for_extracted_element.title,
            new_landmark_for_extracted_element.subtitle,
            new_landmark_for_extracted_element.content,
            new_landmark_for_extracted_element.landmark_type,
            if new_landmark_for_extracted_element.identified { MaturingState::Finished } else { MaturingState::Draft }
        )
    }
}

impl From<MatchedExtractedElementForResource> for MatchedExtractedElementForLandmarkType {
    fn from(extracted_element_for_resource: MatchedExtractedElementForResource) -> Self {
        Self {
            title: format!("Mention de la ressource : {}", extracted_element_for_resource.resource),
            subtitle: "".to_string(),
            extracted_content: extracted_element_for_resource.extracted_content,
            generated_context: extracted_element_for_resource.generated_context,
            landmark_id: extracted_element_for_resource.resource_id,
            landmark_type: ResourceType::Resource
        }
    }
}

impl From<&MatchedExtractedElementForLandmarkType> for NewResource {
    fn from(matched_extracted_element_for_landmark: &MatchedExtractedElementForLandmarkType) -> Self {
        NewResource::new(
            matched_extracted_element_for_landmark.title.clone(),
            matched_extracted_element_for_landmark.subtitle.clone(),
            format!("{} \n\n Enrichi : {}", matched_extracted_element_for_landmark.extracted_content, matched_extracted_element_for_landmark.generated_context),
            ResourceType::Event
        )
    }
}

pub struct ResourceProcessor {
}

#[async_trait]
impl LandmarkProcessor for ResourceProcessor {
    type ExtractedElement = ExtractedElementForResource;
    type MatchedElement = MatchedExtractedElementForResource;

    fn new() -> Self {
        Self {
        }
    }

    async fn extract_elements(
        &self,
        existing_landmarks: &Vec<Landmark>,
        context: &ProcessorContext,
    ) -> Result<Vec<ExtractedElementForResource>, PpdcError> {
        //let existing_resources = existing_landmarks.iter().filter(|landmark| landmark.landmark_type == self.config.landmark_type).map(|landmark| Resource::from((*landmark).clone())).collect::<Vec<Resource>>();
        //let existing_resources = existing_resources.iter().collect::<Vec<&Resource>>();
        let elements = split_trace_in_elements_gpt_request(
            context.trace.content.as_str(), 
            existing_landmarks
            ).await?;

        Ok(elements)
    }
    async fn match_elements(
        &self,
        elements: &Vec<ExtractedElementForResource>,
        existing_landmarks: &Vec<Landmark>,
        context: &ProcessorContext,
    ) -> Result<Vec<MatchedExtractedElementForResource>, PpdcError> {
        let matched_elements = match_elements_and_resources(
            elements, 
            &context.landmarks
        ).await?;
        Ok(matched_elements)
    }
    async fn create_new_landmarks_and_elements(
        &self,
        matched_elements: Vec<MatchedExtractedElementForResource>,
        context: &ProcessorContext,
    ) -> Result<Vec<Landmark>, PpdcError> {
        let new_landmarks: Vec<Landmark> = create_new_landmarks(
            matched_elements, 
            context.user_id, 
            context.trace.id.clone(), 
            context.analysis_resource_id, 
            &context.pool)
            .await?;
        Ok(new_landmarks)
    }
    async fn process(
        &self,
        context: &ProcessorContext,
    ) -> Result<Vec<Landmark>, PpdcError> {
        let elements = self.extract_elements(&context.landmarks, context).await?;
        if elements.is_empty() {
            return Ok(vec![]);
        }
        let matched_elements = self.match_elements(&elements, &context.landmarks, context).await?;
        let new_landmarks = self.create_new_landmarks_and_elements(matched_elements, context).await?;
        Ok(new_landmarks)
        
    }
}
// I will create a new function to split traces.
// This function will split the trace regarding the landmarks types.
// a first function will loop on each lanmark type.
// For each landmark we have a function to split the trace regarding the landmark type.

// Lets think about the function to split trace for resource landmark.
// We first need to ask to identify resources mentioned in the trace.
// A question is if we give the list of resources currently explored by the user.
// What could be useful is to give things that help interpret implicit references (abreviations, "the book")
// Then we get the list of resources mentioned in the trace.
// We ask now to make a mapping.
// Then for unmatched references, we ask to create a new resource.
// Then we persist all the elements and all the resources.


pub async fn create_new_landmarks(
    elements: Vec<MatchedExtractedElementForResource>,
    user_id: Uuid,
    trace_id: Uuid,
    analysis_resource_id: Uuid,
    pool: &DbPool,
) -> Result<Vec<Landmark>, PpdcError> {
    println!("work_analyzer::trace_broker::create_new_landmarks");
    let mut new_landmarks: Vec<Landmark> = vec![];
    for element in elements {
        let created_landmark;
        if element.resource_id.is_none() {
            let new_resource_proposition = get_new_resource_for_extracted_element_from_gpt_request(&element).await?;
            let new_landmark = new_resource_proposition.to_new_landmark();
            created_landmark = create_landmark_for_analysis(new_landmark, user_id, analysis_resource_id, pool)?;
        } else {
            created_landmark = landmark_create_child_and_return(Uuid::parse_str(element.resource_id.clone().unwrap().as_str())?, user_id, analysis_resource_id, pool)?;
        }
        //let element = MatchedExtractedElementForLandmarkType::from(element);
        let _ = element_create_for_trace_landmark_and_analysis(element, trace_id, &created_landmark, analysis_resource_id, user_id, pool);
        new_landmarks.push(created_landmark);
    }
    Ok(new_landmarks)
}

pub fn element_create_for_trace_landmark_and_analysis(
    element: MatchedExtractedElementForResource,
    trace_id: Uuid,
    landmark: &Landmark,
    analysis_resource_id: Uuid,
    user_id: Uuid,
    pool: &DbPool,
) -> Result<Resource, PpdcError> {
    let new_element = element.to_new_resource();
    let created_element = new_element.create(pool)?;
    let mut new_resource_relation = NewResourceRelation::new(created_element.id, trace_id);
    new_resource_relation.relation_type = Some("elmt".to_string());
    new_resource_relation.user_id = Some(user_id);
    new_resource_relation.create(pool)?;
    let mut new_resource_relation = NewResourceRelation::new(created_element.id, landmark.id);
    new_resource_relation.relation_type = Some("elmt".to_string());
    new_resource_relation.user_id = Some(user_id);
    let mut new_analysis_relation= NewResourceRelation::new(created_element.id, analysis_resource_id);
    new_analysis_relation.relation_type = Some("ownr".to_string());
    new_analysis_relation.user_id = Some(user_id);
    new_analysis_relation.create(pool)?;
    Ok(created_element)
}

pub async fn get_new_resource_for_extracted_element_from_gpt_request(
    extracted_element: &MatchedExtractedElementForResource,
) -> Result<NewResourceForExtractedElement, PpdcError> {
    println!("work_analyzer::trace_broker::get_new_resource_for_extracted_element_from_gpt_request");
    let extracted_element_string = serde_json::to_string(&extracted_element)?;
    let system_prompt = format!("System:
    Tu es un moteur de création de nouvelles ressources à partir d'éléments simples.
    Tu dois créer une nouvelle ressource à partir d'un élément simple.
    Si l'élément simple ne permet pas d'identifier le nom de la ressource, donne la description définie qui permet le mieux de l'identifier.
    Si la ressource n'est pas totalement identifiée, indique le champ identified à false.
    Réponds uniquement avec du JSON valide respectant le schéma donné.
    "
    );
    let user_prompt = format!("User:
    Élément simple : {}\n\n", extracted_element_string);
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "title": {"type": "string"},
            "subtitle": {"type": "string"},
            "content": {"type": "string"},
            "author": {"type": "string"},
            "identified": {"type": "boolean"},
            "resource_type": {"type": "string", "enum": ["Book", "Article", "Movie", "Website", "Other"]},
        },
        "required": ["title", "subtitle", "content", "author", "identified", "resource_type"],
        "additionalProperties": false
    });
    let gpt_request_config = GptRequestConfig::new(
        "gpt-4.1-nano".to_string(),
        &system_prompt,
        &user_prompt,
        schema.clone()
    );
    let new_resource_for_extracted_element: NewResourceForExtractedElement = gpt_request_config.execute().await?;
    Ok(new_resource_for_extracted_element)
}

pub async fn match_elements_and_resources(
    elements: &Vec<ExtractedElementForResource>,
    resources: &Vec<Landmark>,
) -> Result<Vec<MatchedExtractedElementForResource>, PpdcError> {
    println!("work_analyzer::trace_broker::match_elements_and_resources");

    let elements_string = serde_json::to_string(&elements)?;
    let resources_string = serde_json::to_string(&resources)?;

    let system_prompt = format!("System:
    Tu es un moteur de correspondance entre éléments simples et ressources.
    Des éléments simples ont été extraits d'un texte utilisateur, et ils sont censés faire référence à des ressources.
    Les ressources actuellement explorées par l'utilisateur sont fournies.
    Pour chaque élément simple, tu dois chercher s'il correspond à une ressource.
    Si un élément simple correspond à une ressource, renvoie le avec l'ID de la ressource.
    Si un élément simple ne correspond à aucune ressource, renvoie le quand même, avec une valeur null pour le resource_id.
    Renvoie tous les éléments simples fournis, même s'ils ne correspondent pas à une ressource.
    Réponds uniquement avec du JSON valide respectant le schéma donné.
    "
    );
    let user_prompt = format!("User:
    Éléments simples : {}\n\n
    Ressources : {}\n\n", elements_string, resources_string);
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "matched_elements": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "resource": {"type": "string"},
                        "extracted_content": {"type": "string"},
                        "generated_context": {"type": "string"},
                        "resource_id": {
                            "anyOf": [
                                {
                                    "type": "string",
                                    "minLength": 1
                                },
                                {
                                    "type": "null"
                                }
                            ]
                        }
                    },
                    "required": [
                        "resource",
                        "extracted_content",
                        "generated_context",
                        "resource_id"
                    ],
                    "additionalProperties": false
                }
            }
        },
        "required": ["matched_elements"],
        "additionalProperties": false
    });
    let gpt_request_config = GptRequestConfig::new(
        "gpt-4.1-nano".to_string(),
        &system_prompt,
        &user_prompt,
        schema.clone()
    );
    let matched_elements: MatchedElementsForResources = gpt_request_config.execute().await?;
    Ok(matched_elements.matched_elements)
}

pub async fn split_trace_in_elements_gpt_request(
    trace: &str,
    resources: &Vec<Landmark>,
) -> Result<Vec<ExtractedElementForResource>, PpdcError> {
    println!("work_analyzer::trace_broker::split_trace_in_elements_gpt_request");
    let resources_string = resources.iter().map(|resource| format!(" Title: {}, Subtitle: {}, Content: {}", resource.title.clone(), resource.subtitle.clone(), resource.content.clone())).collect::<Vec<String>>().join("\n");
    let system_prompt = format!("System:
    Tu es un moteur de décomposition de traces en éléments simples pour une plateforme de suivi de travail.
    Dans cette trace, il est possible que l'utilisateur fasse référence à des ressources (livres, articles, films...).
    Identifie ces ressources, et extrais les parties de la trace qui parlent de ces ressources.
    Ne renvoie des éléments que si une ressource est réellement mentionnée dans la trace.
    Si aucune ressource n'est mentionnée, renvoie une liste vide.

    Pour t'aider à identifier les ressources malgré certaines abréviations et références implicites, je te donne une liste de ressources que l'utilisateur a déjà explorées.
    L'utilisateur peut faire référence à des ressources qui ne sont pas dans cette liste.


    Réponds uniquement avec du JSON valide respectant le schéma donné.
    "
    );
    let user_prompt = format!("User:
    Ressources déjà explorées : {}\n\n
    Trace à décomposer : {}\n\n",
    resources_string, trace);
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "elements": {
                "type": "array", 
                "items": {
                    "type": "object", 
                    "properties": {
                        "resource": {"type": "string"},
                        "extracted_content": {"type": "string"},
                        "generated_context": {"type": "string"}
                    },
                    "required": ["resource", "extracted_content", "generated_context"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["elements"],
        "additionalProperties": false
    });
    let gpt_request_config = GptRequestConfig::new(
        "gpt-4.1-nano".to_string(),
        &system_prompt,
        &user_prompt,
        schema.clone()
    );
    let elements: ExtractedElementsForResources = gpt_request_config.execute().await?;
    Ok(elements.elements)
}
