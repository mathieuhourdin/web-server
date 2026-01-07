use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    interaction::model::NewInteraction,
    resource::{resource_type::ResourceType, NewResource, Resource},
    resource_relation::NewResourceRelation,
};
use crate::openai_handler::gpt_handler::make_gpt_request;
use crate::work_analyzer::context_builder::get_user_biography;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceExtractionProperties {
    pub title: String,
    pub subtitle: String,
    pub interaction_date: Option<NaiveDate>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleElement {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub enriched_content: String,
    pub element_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleElements {
    pub elements: Vec<SimpleElement>,
}

pub async fn qualify_trace(trace: &str) -> Result<(NewResource, Option<NaiveDate>), PpdcError> {
    let system_prompt = format!(
        "System:
    Tu es un asistant de prise de notes et de journaling.
    Détermine un titre et un sous titre en français pour le texte écrit par un utilisateur.
    Si présent, détermine aussi une date où la prise de note a eu lieu.
    Réponds uniquement avec du JSON valide respectant le schéma donné.
    "
    );

    let user_prompt = format!(
        "User:
    Basé sur la transcription audio suivante, extrais uniquement les champs:
    - title (string)
    - subtitle (string)
    - interaction_date (datetime, optional)

    Contenu : {}\n\n",
        trace
    );

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "title": {"type": "string"},
            "subtitle": {"type": "string"},
            "interaction_date": {
                "anyOf": [
                    {"type": "string", "format": "date-time"},
                    {"type": "null"},
                ]
            }
        },
        "required": ["title", "subtitle"]
    });

    let resource_properties: ResourceExtractionProperties =
        make_gpt_request(system_prompt, user_prompt, schema)
            .await
            .unwrap();
    let new_resource = NewResource::new(
        resource_properties.title,
        resource_properties.subtitle,
        trace.to_string(),
        ResourceType::Trace,
    );
    Ok((new_resource, resource_properties.interaction_date))
}

pub async fn split_trace_into_simple_elements(
    trace: &str,
    context: &str,
    user_biography: &str,
) -> Result<Vec<SimpleElement>, PpdcError> {
    let system_prompt = format!("System:
    Tu es un moteur d'analyse de l'activité d'utilisateurs dans une plateforme de gestion de travail.
    Tu décomposes le texte fournit par l'utilisateur en éléments simples.
    Tu peux t'appuyer sur un contexte de travail actuel de l'utilisateur pour comprendre de quoi il parle.
    Cependant, reste proche de ce que dit la trace brute de l'utilisateur pour le découpage et le nommage.
    Utilise surtout le contexte pour enrichir le contenu de l'élément (si certaines abréviations sont utilisées, étend les, explique les, etc.).
    Les éléments simple sont soit des idées soit des événements.
    Chaque élément doit être un objet avec les champs:
    - title (string)
    - subtitle (string)
    - content (string)
    - enriched_content (string)
    - element_type (string)
        - idea
        - event
    Pour le contenu, extrais le texte identifié.
    Pour le enriched_content, enrichis le contenu en français, en t'appuyant sur le contexte.
    Si l'element parle d'une resource ou donnée (livre, article, etc.), indique le dans cet enriched_content.
    Identifie si il s'agit d'une idée ou d'un événement avec le champ type..
    Tu dois être exhaustif dans la décomposition du texte.
    Tu dois éviter de mélanger deux éléments différents dans le même élément.
    Tu peux t'appuyer sur la biographie de l'utilisateur pour mieux comprendre son travail.
    ");

    let user_prompt = format!(
        "User:
    Biographie de l'utilisateur : {}\n\n
    Context : {}\n\n
    Contenu : {}\n\n",
        user_biography, context, trace
    );

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "elements": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "title": {"type": "string"},
                        "subtitle": {"type": "string"},
                        "content": {"type": "string"},
                        "enriched_content": {"type": "string"},
                        "element_type": {"type": "string"},
                    },
                }
            }
        },
        "required": ["elements"]
    });

    let elements: SimpleElements = make_gpt_request(system_prompt, user_prompt, schema)
        .await
        .unwrap();
    Ok(elements.elements)
}

pub fn create_resource_from_simple_element(
    simple_element: &SimpleElement,
    existing_trace_id: Uuid,
    user_id: Uuid,
    analysis_resource_id: Uuid,
    pool: &DbPool,
) -> Result<Resource, PpdcError> {
    let content = format!(
        "Extrait de la trace : {}\n\nEnrichi : {}",
        simple_element.content.clone(),
        simple_element.enriched_content.clone()
    );
    let new_resource = NewResource::new(
        simple_element.title.clone(),
        simple_element.subtitle.clone(),
        content,
        ResourceType::Trace,
    );
    let resource = new_resource.create(pool)?;

    // we create a relation to the trace, with type "elmt". Means that those are elements of the trace.
    let mut new_resource_relation_to_trace =
        NewResourceRelation::new(resource.id, existing_trace_id);
    new_resource_relation_to_trace.user_id = Some(user_id);
    new_resource_relation_to_trace.relation_type = Some("elmt".to_string());
    new_resource_relation_to_trace.create(pool)?;

    // we create a relation to the analysis resource, with type "ownr". Means that those have been created during this analysis.
    let mut new_resource_relation_to_analysis =
        NewResourceRelation::new(resource.id, analysis_resource_id);
    new_resource_relation_to_analysis.user_id = Some(user_id);
    new_resource_relation_to_analysis.relation_type = Some("ownr".to_string());
    new_resource_relation_to_analysis.create(pool)?;

    // we create a new interaction for the resource, with type "auto". Means that this is an automatic creation.
    let mut new_interaction = NewInteraction::new(user_id, resource.id);
    new_interaction.interaction_type = Some("auto".to_string());
    new_interaction.create(pool)?;
    Ok(resource)
}

pub async fn create_element_resources_from_trace(
    trace: Resource,
    user_id: Uuid,
    context: String,
    analysis_resource_id: Uuid,
    pool: &DbPool,
) -> Result<Vec<Resource>, PpdcError> {
    let trace_content = format!(
        "Titre : {}\nSous titre : {}\nContenu : {}",
        trace.title, trace.subtitle, trace.content
    );

    let user_biography = get_user_biography(&user_id, pool)?;

    let elements = split_trace_into_simple_elements(
        trace_content.as_str(),
        context.as_str(),
        user_biography.as_str(),
    )
    .await?;
    let resources = elements
        .iter()
        .map(|element| {
            create_resource_from_simple_element(
                element,
                trace.id,
                user_id,
                analysis_resource_id,
                pool,
            )
            .unwrap()
        })
        .collect();
    Ok(resources)
}
