use uuid::Uuid;
use crate::entities::{
    error::PpdcError,
    resource::{
        Resource,
        NewResource,
        resource_type::ResourceType,
        maturing_state::MaturingState,
    },
    resource_relation::NewResourceRelation,
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use crate::db::DbPool;

#[derive(Serialize, Deserialize, Debug)]
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
}

impl NewLandmark {
    pub fn new(title: String, subtitle: String, content: String, landmark_type: ResourceType, maturing_state: MaturingState) -> NewLandmark {
        Self {
            title,
            subtitle,
            content,
            landmark_type,
            maturing_state,
            publishing_state: "pbsh".to_string(),
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
        let new_resource = self.to_new_resource();
        let created_resource = new_resource.create(pool)?;
        Ok(Landmark::from_resource(created_resource))
    }
}

pub fn create_landmark_with_parent(parent_landmark_id: Uuid, landmark: NewLandmark, user_id: Uuid, analysis_id: Uuid, pool: &DbPool) -> Result<Landmark, PpdcError> {
    let mut parent_landmark = Landmark::find(parent_landmark_id, pool)?;
    let mut landmark = landmark;
    landmark.publishing_state = "pbsh".to_string();
    let landmark = create_landmark_for_analysis(landmark, user_id, analysis_id, pool)?;
    let mut new_resource_relation = NewResourceRelation::new(landmark.id, parent_landmark.id);
    new_resource_relation.relation_type = Some("prnt".to_string());
    new_resource_relation.user_id = Some(user_id);
    new_resource_relation.create(pool)?;
    parent_landmark.publishing_state = "drft".to_string();
    parent_landmark.update(pool)?;
    Ok(landmark)
}

pub fn landmark_create_child_and_return(
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
        parent_landmark.maturing_state
    );
    let landmark = create_landmark_with_parent(parent_landmark_id, landmark, user_id, analysis_id, pool)?;
    Ok(landmark)
}

pub fn create_landmark_for_analysis(landmark: NewLandmark, user_id: Uuid, analysis_id: Uuid, pool: &DbPool) -> Result<Landmark, PpdcError> {
    let landmark = landmark.create(pool)?;
    let mut new_resource_relation = NewResourceRelation::new(landmark.id, analysis_id);
    new_resource_relation.relation_type = Some("ownr".to_string());
    new_resource_relation.user_id = Some(user_id);
    new_resource_relation.create(pool)?;
    Ok(landmark)
}