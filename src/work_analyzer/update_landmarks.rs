use crate::entities::error::PpdcError;
use crate::openai_handler::gpt_handler::make_gpt_request;
use serde::{Deserialize, Serialize};
use crate::work_analyzer::match_elements_and_landmarks::LandmarkPojectionWithElements;
use crate::entities::resource::{Resource, NewResource, resource_type::ResourceType};
use crate::entities::resource_relation::NewResourceRelation;
use crate::entities::interaction::model::NewInteraction;
use crate::db::DbPool;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisEntity {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub progress: i32,
    pub closed: bool,
    pub entity_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub entities: Vec<AnalysisEntity>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateLandmarksResult {
    pub landmarks: Vec<LandmarkPojectionWithElements>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateLandmark {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub progress: i32,
    pub closed: bool,
    pub entity_type: String,
}

pub fn create_landmarks_and_link_elements(landmarks: &Vec<LandmarkPojectionWithElements>, simple_elements: &Vec<Resource>, analysis_resource_id: &Uuid, user_id: &Uuid, pool: &DbPool) -> Result<Vec<Resource>, PpdcError> {

    let created_landmarks = landmarks.iter().map(|landmark| -> Result<Resource, PpdcError> {
        let new_landmark = NewResource::new(
            landmark.title.clone(), 
            landmark.subtitle.clone(), 
            landmark.content.clone(), 
            ResourceType::from_code(landmark.entity_type.as_str())?);
        let created_landmark = new_landmark.create(pool)?;
        let mut new_interaction = NewInteraction::new(user_id.clone(), created_landmark.id);
        new_interaction.interaction_type = Some("anly".to_string());
        new_interaction.interaction_progress = landmark.progress.unwrap_or(0);
        new_interaction.create(pool)?;
        if !landmark.is_new {
            let mut link_to_parent_landmark = NewResourceRelation::new(
                created_landmark.id, 
                Uuid::parse_str(landmark.id.as_str())?);
            link_to_parent_landmark.relation_type = Some("prnt".to_string());
            link_to_parent_landmark.user_id = Some(user_id.clone());
            link_to_parent_landmark.create(pool)?;
        }
        let mut new_resource_relation_to_analysis = NewResourceRelation::new(created_landmark.id, analysis_resource_id.clone());
        new_resource_relation_to_analysis.user_id = Some(user_id.clone());
        new_resource_relation_to_analysis.relation_type = Some("ownr".to_string());
        new_resource_relation_to_analysis.create(pool)?;
        for simple_element in simple_elements {
            let mut new_resource_relation_to_simple_element = NewResourceRelation::new(simple_element.id, created_landmark.id);
            new_resource_relation_to_simple_element.user_id = Some(user_id.clone());
            new_resource_relation_to_simple_element.relation_type = Some("srcs".to_string());
            new_resource_relation_to_simple_element.create(pool)?;
        }
        Ok(created_landmark)
    }).collect::<Result<Vec<Resource>, PpdcError>>()?;
    Ok(created_landmarks)
}

pub async fn update_landmarks(landmarks: &Vec<LandmarkPojectionWithElements>) -> Result<Vec<AnalysisEntity>, PpdcError> {
    let system_prompt = format!("System:
    Tu es un analyste de travail. Tu dois analyser les landmarks prisent par l'utilisateur au cours de son travail (notes de lecture, carnet de bord, etc.) et dégager des entités stables du travail de l'utilisateur.
    Tu maintiens une liste de landmarks existants.
    Pour chaque landmark, tu dois : 
    - mettre à jour le contenu du landmark. Il faut l'enrichir au maximum avec le contenu des éléments simples associés au landmark, et avec l'état précédent.
    - déterminer si le landmark doit être fermé, dans le cas où l'utilisateur semble avoir terminé ou abandonné l'entité.
    - mettre à jour le progress du landmark, en pourcentage de la progression de l'entité.
    - conserver l'id fournie pour ce landmark.

    Les landmarks à mettre à jour sont de plusieurs types : 
    - task (Tâche, chose que l'utilisateur se donne à faire explicitement ou implicitement, souvent dans un horizon temporel défini),
    - qest (Question, questionnement qui revient chez l'utilisateur), 
    - dlvr (Livrable, livrables produits par l'utilisateur (ex : fiche sur une ressource, mémoire, projet)),
    - proc (Processus, processus de travail que l'utilisateur essaie d'adopter),
    - rsrc (Ressource, ressources avec lesquelles l'utilisateur interragit : livres, articles, etc)

    Pour chaque entité, tu dois dégager les champs suivants :
    - id (string) : id du landmark
    - title (string)
    - subtitle (string)
    - content (string) : description de l'entité
    - progress (integer) : progression de l'entité (0 à 100). A quel point la tache a été réalisée, la ressource a été lue, le livrable a été produit, le processus a été adopté, la question a été résolue.
    - closed (boolean) : si l'entité est close (true) ou non (false). La tâche est closed si on considère que l'utilisateur ne reviendra plus dessus (fini ou choix de l'abandonner).
    - entity_type (string)");

    let user_prompt = format!("User:
    Landmarks à mettre à jour : {}\n\n", landmarks.iter().map(|landmark| format!("Id : {}\nTitre : {}\nSous titre : {}\nContenu : {}\nProgress : {}\nClosed : {}\nEntity type : {}", landmark.id, landmark.title, landmark.subtitle, landmark.content, landmark.progress.unwrap_or(0), landmark.closed, landmark.entity_type)).collect::<Vec<_>>().join("\n\n"));

    println!("System prompt: {}", system_prompt);

    println!("User prompt: {}", user_prompt);

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "entities": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string"},
                        "title": {"type": "string"},
                        "subtitle": {"type": "string"},
                        "content": {"type": "string"},
                        "progress": {"type": "integer"},
                        "closed": {"type": "boolean"},
                        "entity_type": {"type": "string"},
                    },
                }
            }
        },
        "required": ["entities"]
    });

    let update_landmarks_result: UpdateLandmarksResult = make_gpt_request(system_prompt, user_prompt, schema).await.unwrap();
    Ok(vec![])
}

pub async fn create_landmarks_from_string_traces(string_traces: String) -> Result<Vec<AnalysisEntity>, PpdcError> {
    let system_prompt = format!("System:
    Tu es un analyste de travail. Tu dois analyser les traces prises par l'utilisateur au cours de son travail (notes de lecture, carnet de bord, etc.) et dégager des entités stables du travail de l'utilisateur.
    Les entités à dégager sont de plusieurs types : 
     - taches (chose que l'utilisateur se donne à faire explicitement ou implicitement, souvent dans un horizon temporel défini. Code : task),
     - questions (questionnement qui revient chez l'utilisateur. Code : qest), 
     - livrables (livrables produits par l'utilisateur (ex : fiche sur une ressource, mémoire, projet. Code : dlvr),
     - processus (processus de travail que l'utilisateur essaie d'adopter. Code : proc),
     - ressources (ressources avec lesquelles l'utilisateur interragit : livres, articles, etc. Code : rsrc)

    Pour chaque entité, tu dois dégager les champs suivants :
    - title (string)
    - subtitle (string)
    - content (string) : description de l'entité
    - progress (integer) : progression de l'entité (0 à 100). A quel point la tache a été réalisée, la ressource a été lue, le livrable a été produit, le processus a été adopté, la question a été résolue.
    - closed (boolean) : si l'entité est close (true) ou non (false). La tâche est closed si on considère que l'utilisateur ne reviendra plus dessus (fini ou choix de l'abandonner).
    - entity_type (string)
        - task
        - qest
        - dlvr
        - proc
        - rsrc
    Pour les ressources, prête une attention particulière au titre. Il peut parfois apparaitre de façon abrégée, ou par les initiales du titre.
    Le titre doit être le titre complet de la ressource, sans abréviation. Conserve l'unicité de la ressource.
    Prête attention à ne pas dupliquer les entités.
    Réponds uniquement avec du JSON valide respectant le schéma donné.
    ");

    let user_prompt = format!("User:
    Contenu à analyser : {}\n\n", string_traces);

    println!("System prompt: {}", system_prompt);

    println!("User prompt: {}", user_prompt);

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "entities": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "title": {"type": "string"},
                        "subtitle": {"type": "string"},
                        "content": {"type": "string"},
                        "progress": {"type": "integer"},
                        "closed": {"type": "boolean"},
                        "entity_type": {"type": "string"},
                    },
                }
            }
        },
        "required": ["entities"]
    });

    let analysis_result: AnalysisResult = make_gpt_request(system_prompt, user_prompt, schema).await.unwrap();
    Ok(analysis_result.entities)
}