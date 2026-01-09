use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    interaction::model::NewInteraction,
    resource::{resource_type::ResourceType, NewResource, Resource},
    resource_relation::NewResourceRelation,
};
use crate::work_analyzer::trace_broker::broke_for_resource::split_trace_in_elements_for_resources_landmarks;
use crate::openai_handler::gpt_responses_handler::make_gpt_request;
use crate::work_analyzer::context_builder::{get_ontology_context, get_user_biography};
use chrono::NaiveDate;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod broke_for_resource;

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
pub struct ExtractedElementForResource {
    pub resource: String,
    pub extracted_content: String,
    pub generated_context: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleElementWithResource {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub enriched_content: String,
    pub element_type: String,
    pub resource_id: String,
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
                    {"type": "string", "format": "date"},
                    {"type": "null"},
                ]
            }
        },
        "required": ["title", "subtitle", "interaction_date"],
        "additionalProperties": false
    });

    let resource_properties: ResourceExtractionProperties =
        make_gpt_request(system_prompt, user_prompt, schema)
            .await
            ?;
    let new_resource = NewResource::new(
        resource_properties.title,
        resource_properties.subtitle,
        trace.to_string(),
        ResourceType::Trace,
    );
    Ok((new_resource, resource_properties.interaction_date))
}

pub async fn process_trace(
    trace: &Resource,
    user_id: Uuid,
    landmarks: &Vec<Resource>,
    analysis_resource_id: Uuid,
    pool: &DbPool,
) -> Result<Vec<Resource>, PpdcError> {
    let resources_landmarks = landmarks.into_iter().filter(|landmark| landmark.resource_type == ResourceType::Resource).collect::<Vec<&Resource>>();
    let elements = split_trace_in_elements_for_resources_landmarks(user_id, analysis_resource_id, trace.content.as_str(), &resources_landmarks, pool).await?;
    Ok(elements)
}

pub async fn split_trace_into_simple_elements(
    trace: &str,
    context: &str,
    user_biography: &str,
) -> Result<Vec<SimpleElement>, PpdcError> {
    println!("work_analyzer::trace_broker::split_trace_into_simple_elements: Splitting trace into simple elements for trace: {}, context: {}, user biography: {}", trace, context, user_biography);
    let ontology_context = get_ontology_context();
    let system_prompt = format!("System:

    Ontologie de la plateforme : {}\n\n

    Tu es un moteur de décomposition de traces en éléments simples pour une plateforme de suivi de travail.

    Objectif
    À partir d’une trace écrite par l’utilisateur (journal, notes, réflexions, etc.), tu dois la découper en éléments simples. Un élément simple correspond à une idée ou un événement (action, fait) suffisamment atomique pour être utile à analyser séparément.

    Tu peux recevoir en plus un contexte de travail et/ou une biographie de l’utilisateur. Utilise ce contexte uniquement pour:
    - clarifier des abréviations ou références implicites,
    - enrichir la reformulation (enriched_content).
    Ne l’utilise pas pour inventer des éléments qui ne sont pas présents dans la trace.

    Règles de décomposition
    - Parcours la trace dans l’ordre, et produis une liste d’éléments simples dans le même ordre.
    - Tu dois être aussi exhaustif que possible, sans inventer de contenu non présent (même implicitement, il doit être raisonnablement déductible du texte).
    - Ne fusionne pas des éléments qui pourraient être utiles séparément.
    - Ne renvoie pas la trace entière dans le contenu de l'élément.

    Ontologie des éléments
    Chaque élément doit être un objet avec les champs suivants:

    - title (string, en français):
      Titre court qui résume l’idée ou l’événement (quelques mots ou une courte phrase).

    - subtitle (string, en français):
      Contexte ou précision complémentaire, plus long que le titre mais plus court que le content complet. Peut être vide si tu n’as rien d’utile à ajouter.

    - content (string, en français):
      Reformulation concise et fidèle de ce qui est dit dans la trace pour cet élément précis. Reste très proche du texte d’origine.

    - enriched_content (string, en français):
      Version enrichie de content:
      - explicite les abréviations, acronymes ou références si le contexte le permet,
      - précise les ressources mentionnées (livre, article, outil, personne, etc.),
      - peut lier l’élément à des thèmes de travail de l’utilisateur (sans inventer de nouveaux faits).

    - element_type (string, valeur parmi):
      - 'Idea'           : idée, réflexion, hypothèse, prise de position.
      - 'Event'          : action réalisée, événement factuel, chose qui s’est produite.
      - 'GeneralComment' : remarque générale, meta-commentaire, aparté peu structuré.
      - 'Feeling'        : émotion ou ressenti de l’utilisateur (positif, négatif, ambivalent, etc.).

    Choisis le type le plus spécifique possible:
    - si le cœur de l’élément est un ressenti → 'Feeling', même s’il mentionne un événement.
    - si c’est une action ou un fait concret → 'Event'.
    - si c’est surtout une idée/insight → 'Idea'.
    - si c’est un commentaire vague ou peu structuré → 'GeneralComment'.

    Contraintes de sortie
    - Tu dois répondre uniquement avec du JSON valide respectant le schéma fourni (liste d’objets élément simple).
    - Tous les textes (title, subtitle, content, enriched_content) doivent être rédigés en français.
    - Les noms de champs et les valeurs de element_type doivent rester en anglais et ne jamais être traduits.
    ", ontology_context);

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
                        "element_type": {"type": "string", "enum": ["Idea", "Event", "GeneralComment", "Feeling"]},
                    },
                    "required": ["title", "subtitle", "content", "enriched_content", "element_type"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["elements"],
        "additionalProperties": false
    });

    let elements: SimpleElements = make_gpt_request(system_prompt, user_prompt, schema)
        .await
        ?;
    Ok(elements.elements)
}

pub fn create_resource_from_simple_element(
    simple_element: &SimpleElement,
    existing_trace_id: Uuid,
    user_id: Uuid,
    analysis_resource_id: Uuid,
    pool: &DbPool,
) -> Result<Resource, PpdcError> {
    println!("work_analyzer::trace_broker::create_resource_from_simple_element: Creating resource from simple element: {:?}, existing trace id: {}, user id: {}, analysis resource id: {}", simple_element, existing_trace_id, user_id, analysis_resource_id);
    let content = format!(
        "Extrait de la trace : {}\n\nEnrichi : {}",
        simple_element.content.clone(),
        simple_element.enriched_content.clone()
    );
    let resource_type = ResourceType::from_full_text(simple_element.element_type.as_str())?;
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
    println!("work_analyzer::trace_broker::create_element_resources_from_trace: Creating element resources from trace: {:?}, user id: {}, context: {}, analysis resource id: {}", trace, user_id, context, analysis_resource_id);
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
        })
        .collect::<Result<Vec<Resource>, PpdcError>>()?;
    Ok(resources)
}
