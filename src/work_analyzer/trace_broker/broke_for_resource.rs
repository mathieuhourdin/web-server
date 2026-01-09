use crate::entities::error::PpdcError;
use crate::entities::resource::{NewResource, Resource, maturing_state::MaturingState};
use crate::entities::resource_relation::NewResourceRelation;
use crate::entities::resource::resource_type::ResourceType;
use crate::entities::landmark::{Landmark, NewLandmark, create_landmark_with_parent};
use crate::openai_handler::gpt_responses_handler::make_gpt_request;
use crate::work_analyzer::trace_broker::{SimpleElement, SimpleElements};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use crate::db::DbPool;

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedElementForResource {
    pub resource: String,
    pub extracted_content: String,
    pub generated_context: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatchedExtractedElementForResource {
    pub resource: String,
    pub extracted_content: String,
    pub generated_context: String,
    pub resource_id: Option<String>,
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

pub async fn split_trace_in_elements_for_resources_landmarks(
    user_id: Uuid,
    analysis_resource_id: Uuid,
    trace: &str,
    resources: &Vec<&Resource>,
    pool: &DbPool,
) -> Result<Vec<Resource>, PpdcError> {
    println!("work_analyzer::trace_broker::split_trace_in_elements_for_resources_landmarks");
    let elements = split_trace_in_elements_gpt_request(trace, resources).await?;
    if elements.is_empty() {
        return Ok(vec![]);
    }
    let matched_elements = match_elements_and_resources(&elements, resources).await?;
    let new_resources = create_new_resources(&matched_elements, user_id, analysis_resource_id, pool).await?;

    Ok(new_resources)
}

pub async fn create_new_resources(
    elements: &Vec<MatchedExtractedElementForResource>,
    user_id: Uuid,
    analysis_resource_id: Uuid,
    pool: &DbPool,
) -> Result<Vec<Resource>, PpdcError> {
    println!("work_analyzer::trace_broker::create_new_resources");
    for element in elements {
        let mut created_landmark;
        if element.resource_id.is_none() {
            let new_landmark_proposition = get_new_resource_for_extracted_element_from_gpt_request(element).await?;
            let new_landmark = NewLandmark::new(
                new_landmark_proposition.title,
                format!("By {} - {}", new_landmark_proposition.author, new_landmark_proposition.subtitle),
                new_landmark_proposition.content,
                ResourceType::Resource,
                if new_landmark_proposition.identified { MaturingState::Finished } else { MaturingState::Draft },
            );
            created_landmark = new_landmark.create(pool)?;
            let mut new_resource_relation = NewResourceRelation::new(created_landmark.id, analysis_resource_id);
            new_resource_relation.user_id = Some(user_id);
            new_resource_relation.relation_type = Some("ownr".to_string());
            new_resource_relation.create(pool)?;
        } else {
            let landmark = Landmark::find(Uuid::parse_str(element.resource_id.clone().unwrap().as_str())?, pool)?;
            let new_landmark = NewLandmark::new(
                landmark.title,
                landmark.subtitle,
                landmark.content,
                landmark.landmark_type,
                landmark.maturing_state,
            );
            created_landmark = create_landmark_with_parent(landmark.id, new_landmark, user_id, pool)?;
            let mut new_resource_relation = NewResourceRelation::new(created_landmark.id, analysis_resource_id);
            new_resource_relation.user_id = Some(user_id);
            new_resource_relation.relation_type = Some("ownr".to_string());
            new_resource_relation.create(pool)?;
        }
        let new_element = NewResource::new(
            format!("Mention de la ressource : {}", created_landmark.title),
            String::new(),
            format!("{} \n\n Enrichi : {}", element.extracted_content, element.generated_context),
            ResourceType::Event,
        );
        let created_element = new_element.create(pool)?;
        let mut new_resource_relation = NewResourceRelation::new(created_element.id, created_landmark.id);
        new_resource_relation.relation_type = Some("elmt".to_string());
        new_resource_relation.user_id = Some(user_id);
        new_resource_relation.create(pool)?;
        let mut new_resource_relation = NewResourceRelation::new(created_element.id, analysis_resource_id);
        new_resource_relation.user_id = Some(user_id);
        new_resource_relation.relation_type = Some("ownr".to_string());
        new_resource_relation.create(pool)?;
    }
    Ok(vec![])
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
    let new_resource_for_extracted_element: NewResourceForExtractedElement = make_gpt_request(system_prompt, user_prompt, schema)
        .await?;
    Ok(new_resource_for_extracted_element)
}

pub async fn match_elements_and_resources(
    elements: &Vec<ExtractedElementForResource>,
    resources: &Vec<&Resource>,
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
    let matched_elements: MatchedElementsForResources = make_gpt_request(system_prompt, user_prompt, schema)
        .await?;
    Ok(matched_elements.matched_elements)
}

pub async fn split_trace_in_elements_gpt_request(
    trace: &str,
    resources: &Vec<&Resource>,
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
    let elements: ExtractedElementsForResources = make_gpt_request(system_prompt, user_prompt, schema)
        .await?;
    Ok(elements.elements)
}
