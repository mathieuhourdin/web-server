use crate::entities::{
    analysis,
    resource::Resource,
    resource_relation::ResourceRelation,
    user::User
};
use crate::db::DbPool;
use crate::entities::error::PpdcError;
use uuid::Uuid;


pub fn get_landmarks_from_analysis(analysis_resource_id: Option<Uuid>, pool: &DbPool) -> Result<Vec<Resource>, PpdcError> {
    if analysis_resource_id.is_none() {
        return Ok(vec![]);
    }
    let landmarks = ResourceRelation::find_origin_for_resource(analysis_resource_id.unwrap(), pool)?;
    let landmarks = landmarks
    .iter()
    .filter(|landmark| landmark.resource_relation.relation_type == "ownr".to_string())
    .map(|landmark| landmark.origin_resource.clone()).collect::<Vec<_>>();
    Ok(landmarks)
}

pub fn get_user_biography(user_id: &Uuid, pool: &DbPool) -> Result<String, PpdcError> {
    let user = User::find(user_id, &pool)?;
    let biography = user.biography.unwrap_or_default();
    Ok(biography)
}

pub fn build_context(user_id: &Uuid, last_analysis_id: Option<Uuid>, pool: &DbPool) -> Result<String, PpdcError> {

    // user biography
    let user = User::find(user_id, &pool)?;
    let biography = user.biography.unwrap_or_default();
    let biography_context = format!("Biographie de l'utilisateur : {}\n", biography);

    // context of the user's work
    let landmarks = get_landmarks_from_analysis(last_analysis_id, &pool)?;
    let string_context = landmarks
        .iter()
        .map(
            |resource| format!(
                "Titre : {}\nSous titre : {}\nContenu : {}", 
                resource.title.clone(), 
                resource.subtitle.clone(), 
                resource.content.clone())
            )
            .collect::<Vec<_>>()
            .join("\n");
    let context_of_work = format!("Contexte de travail de l'utilisateur : {}\n", string_context);

    let full_context = format!("{} {}", biography_context, context_of_work);


    Ok(full_context)
}