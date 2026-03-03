use crate::db::DbPool;
use crate::entities::interaction::model::Interaction;
use crate::entities_v2::error::PpdcError;
use crate::schema::*;
use chrono::NaiveDateTime;
use diesel;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::resource::entity_type::EntityType;
use crate::entities::resource::maturing_state::MaturingState;
use crate::entities::resource::resource_type::ResourceType;

#[derive(Serialize, Deserialize, Clone, Queryable, Selectable, AsChangeset, Debug)]
#[diesel(table_name=resources)]
pub struct Resource {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub external_content_url: Option<String>,
    pub comment: Option<String>,
    pub image_url: Option<String>,
    pub resource_type: ResourceType,
    pub maturing_state: MaturingState,
    pub publishing_state: String,
    pub is_external: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub entity_type: EntityType,
    pub resource_subtype: Option<String>,
}

#[derive(Deserialize, Insertable, Queryable, AsChangeset)]
#[diesel(table_name=resources)]
pub struct NewResource {
    pub title: String,
    pub subtitle: String,
    pub content: Option<String>,
    pub external_content_url: Option<String>,
    pub comment: Option<String>,
    pub image_url: Option<String>,
    pub resource_type: Option<ResourceType>,
    pub maturing_state: Option<MaturingState>,
    pub publishing_state: Option<String>,
    pub is_external: Option<bool>,
    pub entity_type: Option<EntityType>,
    pub resource_subtype: Option<String>,
}

#[derive(Deserialize, Insertable, Queryable, AsChangeset)]
#[diesel(table_name=resources)]
pub struct NewResourceDto {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub content: Option<String>,
    pub external_content_url: Option<String>,
    pub comment: Option<String>,
    pub image_url: Option<String>,
    pub resource_type: Option<ResourceType>,
    pub maturing_state: Option<MaturingState>,
    pub publishing_state: Option<String>,
    pub is_external: Option<bool>,
    pub entity_type: Option<EntityType>,
    pub resource_subtype: Option<String>,
}

impl Resource {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Resource, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let result = resources::table
            .filter(resources::id.eq(id))
            .first(&mut conn)?;
        Ok(result)
    }

    pub fn find_resource_author_interaction(
        &self,
        pool: &DbPool,
    ) -> Result<Interaction, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let result: Interaction = interactions::table
            .filter(interactions::interaction_type.eq("outp"))
            .filter(interactions::resource_id.eq(self.id))
            .first(&mut conn)?;
        Ok(result)
    }

    pub fn update(self, pool: &DbPool) -> Result<Resource, PpdcError> {
        let id = self.id;
        let new_resource = NewResource::from(self);
        new_resource.update(&id, pool)
    }

    pub fn delete(self, pool: &DbPool) -> Result<Resource, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        diesel::delete(resources::table)
            .filter(resources::id.eq(self.id))
            .execute(&mut conn)?;
        Ok(self)
    }

    pub fn is_entity_type(&self, entity_type: EntityType) -> bool {
        self.entity_type == entity_type
    }

    pub fn is_public_post(&self) -> bool {
        self.is_entity_type(EntityType::PublicPost)
    }

    pub fn is_journal(&self) -> bool {
        self.is_entity_type(EntityType::Journal)
    }

    pub fn is_trace(&self) -> bool {
        self.is_entity_type(EntityType::Trace)
    }

    pub fn is_trace_mirror(&self) -> bool {
        self.is_entity_type(EntityType::TraceMirror)
    }

    pub fn is_element(&self) -> bool {
        self.is_entity_type(EntityType::Element)
    }

    pub fn is_landmark(&self) -> bool {
        self.is_entity_type(EntityType::Landmark)
    }

    pub fn is_landscape_analysis(&self) -> bool {
        self.is_entity_type(EntityType::LandscapeAnalysis)
    }

    pub fn is_lens(&self) -> bool {
        self.is_entity_type(EntityType::Lens)
    }
}

impl NewResource {
    pub fn new(
        title: String,
        subtitle: String,
        content: String,
        resource_type: ResourceType,
    ) -> NewResource {
        println!("Vaule for resource type : {}", resource_type.to_code());
        Self {
            title,
            subtitle,
            content: Some(content),
            external_content_url: None,
            comment: None,
            image_url: None,
            resource_type: Some(resource_type),
            entity_type: Some(resource_type.to_entity_type()),
            maturing_state: Some(MaturingState::Draft),
            publishing_state: Some("pbsh".to_string()),
            is_external: Some(false),
            resource_subtype: None,
        }
    }

    pub fn create(mut self, pool: &DbPool) -> Result<Resource, PpdcError> {
        if self.entity_type.is_none() {
            self.entity_type = self
                .resource_type
                .map(|resource_type| resource_type.to_entity_type());
        }
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let result = diesel::insert_into(resources::table)
            .values(&self)
            .get_result(&mut conn)?;
        Ok(result)
    }

    pub fn update(self, id: &Uuid, pool: &DbPool) -> Result<Resource, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let result = diesel::update(resources::table)
            .filter(resources::id.eq(id))
            .set(&self)
            .get_result(&mut conn)?;
        Ok(result)
    }
}

impl From<Resource> for NewResource {
    fn from(resource: Resource) -> Self {
        Self {
            title: resource.title,
            subtitle: resource.subtitle,
            content: Some(resource.content),
            external_content_url: resource.external_content_url,
            comment: resource.comment,
            image_url: resource.image_url,
            resource_type: Some(resource.resource_type),
            entity_type: Some(resource.entity_type),
            maturing_state: Some(resource.maturing_state),
            publishing_state: Some(resource.publishing_state),
            is_external: Some(resource.is_external),
            resource_subtype: resource.resource_subtype,
        }
    }
}
