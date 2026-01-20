use uuid::Uuid;
use crate::entities::{
    error::{ErrorType, PpdcError},
    resource::{
        Resource,
        NewResource,
        resource_type::ResourceType,
        maturing_state::MaturingState,
    },
    resource_relation::{NewResourceRelation, ResourceRelation},
    interaction::model::{NewInteraction, Interaction},
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use axum::extract::{Extension, Json, Path};
use crate::db::DbPool;
use axum::debug_handler;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Landmark {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub external_content_url: Option<String>,
    pub comment: Option<String>,
    pub image_url: Option<String>,
    pub landmark_type: ResourceType,
    pub maturing_state: MaturingState,
    pub publishing_state: String,
    pub category_id: Option<Uuid>,
    pub is_external: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NewLandmark {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub landmark_type: ResourceType,
    pub maturing_state: MaturingState,
    pub publishing_state: String,
    pub analysis_id: Uuid,
    pub user_id: Uuid,
    pub parent_id: Option<Uuid>,
}


impl Landmark {
    pub fn to_resource(self) -> Resource {
        Resource {
            id: self.id,
            title: self.title,
            subtitle: self.subtitle,
            content: self.content,
            external_content_url: self.external_content_url,
            comment: self.comment,
            image_url: self.image_url,
            resource_type: self.landmark_type,
            maturing_state: self.maturing_state,
            publishing_state: self.publishing_state,
            category_id: self.category_id,
            is_external: self.is_external,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
    pub fn from_resource(resource: Resource) -> Self {
        Self {
            id: resource.id,
            title: resource.title,
            subtitle: resource.subtitle,
            content: resource.content,
            external_content_url: resource.external_content_url,
            comment: resource.comment,
            image_url: resource.image_url,
            landmark_type: resource.resource_type,
            maturing_state: resource.maturing_state,
            publishing_state: resource.publishing_state,
            category_id: resource.category_id,
            is_external: resource.is_external,
            created_at: resource.created_at,
            updated_at: resource.updated_at,
        }
    }
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Landmark, PpdcError> {
        let result = Resource::find(id, pool)?;
        Ok(Landmark::from_resource(result))
    }
    pub fn update(self, pool: &DbPool) -> Result<Landmark, PpdcError> {
        let result = self.to_resource();
        let updated_resource = result.update(pool)?;
        Ok(Landmark::from_resource(updated_resource))
    }

    pub fn get_analysis(self, pool: &DbPool) -> Result<Resource, PpdcError> {
        let resource_relations = ResourceRelation::find_target_for_resource(self.id, pool)?;
        let analysis = resource_relations
            .into_iter()
            .find(|relation| {
                relation.resource_relation.relation_type == "ownr".to_string()
                && relation.target_resource.resource_type == ResourceType::Analysis
            })
            .map(|relation| relation.target_resource)
            .ok_or(PpdcError::new(400, ErrorType::ApiError, "Analysis not found".to_string()))
            ?;
        Ok(analysis)
    }

    pub fn find_all_up_to_date_for_user(user_id: Uuid, pool: &DbPool) -> Result<Vec<Landmark>, PpdcError> {
        let landmarks_types = [
            ResourceType::Resource,
            ResourceType::Task
        ];
        let mut landmarks = vec![];
        for landmark_type in landmarks_types {
            let interactions = Interaction::find_landmarks_for_user_by_type(user_id, landmark_type, pool)?;
            let landmarks_for_type = interactions.into_iter().map(|interaction| Landmark::from_resource(interaction.resource)).collect::<Vec<Landmark>>();
            landmarks.extend(landmarks_for_type);
        }
        Ok(landmarks)
    }

    pub fn get_for_landscape_analysis(landscape_analysis_id: Uuid, pool: &DbPool) -> Result<Vec<Landmark>, PpdcError> {
        let resource_relations = ResourceRelation::find_origin_for_resource(landscape_analysis_id, pool)?;
        let landmarks = resource_relations
            .into_iter()
            .filter(|relation| {
                relation.origin_resource.resource_type == ResourceType::Resource
                || relation.origin_resource.resource_type == ResourceType::Task
            })
            .map(|relation| Landmark::from_resource(relation.origin_resource))
            .collect::<Vec<Landmark>>();
        Ok(landmarks)
    }
}

impl NewLandmark {
    pub fn new(
        title: String, 
        subtitle: String, 
        content: String, 
        landmark_type: ResourceType, 
        maturing_state: MaturingState,
        analysis_id: Uuid,
        user_id: Uuid,
        parent_id: Option<Uuid>,
    ) -> NewLandmark {
        Self {
            title,
            subtitle,
            content,
            landmark_type,
            maturing_state,
            publishing_state: "pbsh".to_string(),
            analysis_id,
            user_id,
            parent_id
        }
    }

    pub fn to_new_resource(self) -> NewResource {
        NewResource {
            title: self.title,
            subtitle: self.subtitle,
            content: Some(self.content),
            external_content_url: None,
            comment: None,
            image_url: None,
            resource_type: Some(self.landmark_type),
            maturing_state: Some(self.maturing_state),
            publishing_state: Some(self.publishing_state),
            category_id: None,
            is_external: None,
        }
    }
    pub fn create(self, pool: &DbPool) -> Result<Landmark, PpdcError> {
        let analysis_id = self.analysis_id;
        let user_id = self.user_id;
        let parent_id = self.parent_id;

        // create the landmark with all positive flags
        let mut landmark = self;
        landmark.publishing_state = "fnsh".to_string();
        landmark.maturing_state = MaturingState::Finished;
        let new_resource = landmark.to_new_resource();
        let created_resource = new_resource.create(pool)?;
        let landmark = Landmark::from_resource(created_resource);

        // Create relation with analysis
        let mut new_resource_relation = NewResourceRelation::new(landmark.id, analysis_id);
        new_resource_relation.relation_type = Some("ownr".to_string());
        new_resource_relation.user_id = Some(user_id);
        new_resource_relation.create(pool)?;


        if let Some(parent_id) = parent_id {

            // create the parent relation to the parent landmark
            let mut new_resource_relation = NewResourceRelation::new(landmark.id, parent_id);
            new_resource_relation.relation_type = Some("prnt".to_string());
            new_resource_relation.user_id = Some(user_id);
            new_resource_relation.create(pool)?;

            // update the parent landmark to the draft state
            let mut parent_landmark = Landmark::find(parent_id, pool)?;
            parent_landmark.publishing_state = "drft".to_string();
            parent_landmark.maturing_state = MaturingState::Draft;
            parent_landmark.update(pool)?;
        }

        // link the landmark to the user with the interaction
        let mut new_interaction = NewInteraction::new(user_id, landmark.id);
        new_interaction.interaction_type = Some("anly".to_string());
        new_interaction.create(pool)?;
        Ok(landmark)
    }
}

impl From<Resource> for Landmark {
    fn from(resource: Resource) -> Self {
        Landmark::from_resource(resource)
    }
}

#[debug_handler]
pub async fn get_landmarks_for_user_route(
    Extension(pool): Extension<DbPool>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Vec<Landmark>>, PpdcError> {
    let landmarks = Landmark::find_all_up_to_date_for_user(user_id, &pool)?;
    Ok(Json(landmarks))
}

pub fn landmark_create_copy_child_and_return(
    parent_landmark_id: Uuid,
    user_id: Uuid,
    analysis_id: Uuid,
    pool: &DbPool,
) -> Result<Landmark, PpdcError> {
    let parent_landmark = Landmark::find(parent_landmark_id, pool)?;
    let landmark = NewLandmark::new(
        parent_landmark.title, 
        parent_landmark.subtitle, 
        parent_landmark.content, 
        parent_landmark.landmark_type, 
        parent_landmark.maturing_state,
        analysis_id,
        user_id,
        Some(parent_landmark_id)

    );
    let landmark = landmark.create(pool)?;
    Ok(landmark)
}