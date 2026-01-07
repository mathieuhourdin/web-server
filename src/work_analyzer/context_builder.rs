use crate::db::DbPool;
use crate::entities::error::PpdcError;
use crate::entities::{resource::Resource, resource_relation::ResourceRelation, user::User};
use uuid::Uuid;

pub fn get_landmarks_from_analysis(
    analysis_resource_id: Option<Uuid>,
    pool: &DbPool,
) -> Result<Vec<Resource>, PpdcError> {
    if analysis_resource_id.is_none() {
        return Ok(vec![]);
    }
    let landmarks =
        ResourceRelation::find_origin_for_resource(analysis_resource_id.unwrap(), pool)?;
    let landmarks = landmarks
        .iter()
        .filter(|landmark| landmark.resource_relation.relation_type == "ownr".to_string())
        .map(|landmark| landmark.origin_resource.clone())
        .collect::<Vec<_>>();
    Ok(landmarks)
}

pub fn get_user_biography(user_id: &Uuid, pool: &DbPool) -> Result<String, PpdcError> {
    let user = User::find(user_id, &pool)?;
    let biography = user.biography.unwrap_or_default();
    Ok(biography)
}

pub fn get_ontology_context() -> String {
    let ontology_context = "Principe général de la plateforme

        La plateforme aide l'utilisateur à suivre, structurer et revisiter son travail dans le temps à partir de ses traces écrites (journal, notes, idées, brouillons).

        À partir de ces traces, la plateforme :
        - extrait des éléments simples (actions et idées unitaires),
        - les relie à des repères stables du travail de l'utilisateur (landmarks),
        - permet de retrouver toutes les idées liées à une question, toutes les actions liées à une tâche ou un livrable, et de voir l'évolution du travail dans le temps.

        L'objectif est de faire émerger un petit nombre de repères clairs et durables, pas de tout sur-structurer.

        Le pipeline de traitement des traces contient les étapes suivantes :
        1. extraction des éléments simples à partir des traces
        2. matching des éléments simples aux landmarks existants et création de nouveaux landmarks si nécessaire
        3. mise à jour des landmarks existants

        Ontologie

        O. Trace
        - Représente une trace écrite de l'utilisateur.
        - Champs principaux :
          - content : texte brut en français écrit par l'utilisateur

        1. ElementSimple (Element)
        - Représente une action ou une idée unitaire extraite d’une trace utilisateur.
        - Champs principaux :
          - simple_element_id : identifiant stable (string)
          - type : 'action' ou 'idea'
          - content : texte brut en français décrivant l’action ou l’idée
        - Un ElementSimple doit être aussi atomique que possible.

        2. Landmark
        - Représente un repère stable dans le travail de l’utilisateur.
        - type ∈ {
            'Task',       // Tâche, chose plus ou moins générique que l'utilisateur se donne à faire explicitement ou implicitement
            'Question',   // question de recherche / problématique ouverte
            'Deliverable',// production attendue (chapitre, article, fiche de lecture, rapport…)
            'Resource',   // ressource mobilisée (livre, article, film…)
            'Process'     // façon de travailler, routine, méthode
          }
        - Champs textuels (en français) :
          - title : titre court et explicite
          - subtitle : sous-titre / contexte optionnel
          - content : description plus détaillée si nécessaire
        - Un Landmark doit rester pertinent sur plusieurs jours ou semaines, pas seulement à l’instant T.

        3. Relation ElementSimple ↔ Landmark
        - Un ElementSimple peut être relié à 0, 1 ou plusieurs Landmarks.
        - Un Landmark peut être relié à 0, 1 ou plusieurs ElementsSimples.
        - Il faut privilégier la réutilisation de Landmarks existants plutôt que créer des doublons, et regrouper plusieurs ElementsSimples sous un même Landmark quand ils décrivent le même repère.

        Langue et conventions

        - Tous les textes destinés à l’utilisateur (title, subtitle, content, résumés…) doivent être en français.
        - Tous les identifiants techniques et valeurs d’énumération (noms de champs JSON, simple_element_id, landmark_id, kind, 'task', 'question', 'deliverable', 'resource', 'process', etc.) sont en anglais et ne doivent jamais être traduits.
        - Les réponses doivent toujours respecter strictement les schémas JSON fournis.
        ".to_string();
    ontology_context
}

pub fn build_context(
    user_id: &Uuid,
    last_analysis_id: Option<Uuid>,
    pool: &DbPool,
) -> Result<String, PpdcError> {
    // user biography
    let user = User::find(user_id, &pool)?;
    let biography = user.biography.unwrap_or_default();
    let biography_context = format!("Biographie de l'utilisateur : {}\n", biography);

    // context of the user's work
    let landmarks = get_landmarks_from_analysis(last_analysis_id, &pool)?;
    let string_context = landmarks
        .iter()
        .map(|resource| {
            format!(
                "Titre : {}\nSous titre : {}\nContenu : {}",
                resource.title.clone(),
                resource.subtitle.clone(),
                resource.content.clone()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let context_of_work = format!(
        "Contexte de travail de l'utilisateur : {}\n",
        string_context
    );

    let full_context = format!("{} {}", biography_context, context_of_work);

    Ok(full_context)
}
