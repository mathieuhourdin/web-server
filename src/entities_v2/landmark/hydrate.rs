use uuid::Uuid;

use super::model::{Landmark, LandmarkType, LandmarkWithParentsAndElements, NewLandmark};
use crate::db::DbPool;
use crate::entities::error::{ErrorType, PpdcError};
use crate::entities::resource::{
    entity_type::EntityType, resource_type::ResourceType, NewResource, Resource,
};
use crate::entities::resource_relation::ResourceRelation;
use crate::entities_v2::element::model::Element;

impl LandmarkType {
    pub fn from_resource_type(resource_type: ResourceType) -> LandmarkType {
        match resource_type {
            ResourceType::Resource => LandmarkType::Resource,
            ResourceType::Theme => LandmarkType::Theme,
            ResourceType::Author => LandmarkType::Author,
            _ => LandmarkType::Resource,
        }
    }
    pub fn to_resource_type(self) -> ResourceType {
        match self {
            LandmarkType::Resource => ResourceType::Resource,
            LandmarkType::Theme => ResourceType::Theme,
            LandmarkType::Author => ResourceType::Author,
        }
    }
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
            resource_type: self.landmark_type.to_resource_type(),
            entity_type: EntityType::Landmark,
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
            landmark_type: LandmarkType::from_resource_type(resource.resource_type),
            maturing_state: resource.maturing_state,
            publishing_state: resource.publishing_state,
            category_id: resource.category_id,
            is_external: resource.is_external,
            created_at: resource.created_at,
            updated_at: resource.updated_at,
        }
    }

    pub fn find_parent(&self, pool: &DbPool) -> Result<Option<Landmark>, PpdcError> {
        let resource_relations = ResourceRelation::find_target_for_resource(self.id, pool)?;
        let parent = resource_relations
            .into_iter()
            .find(|relation| relation.resource_relation.relation_type == "prnt")
            .map(|relation| Landmark::from_resource(relation.target_resource));
        Ok(parent)
    }

    pub fn find_elements(&self, pool: &DbPool) -> Result<Vec<Element>, PpdcError> {
        let resource_relations = ResourceRelation::find_origin_for_resource(self.id, pool)?;
        let elements = resource_relations
            .into_iter()
            .filter(|relation| relation.resource_relation.relation_type == "elmt")
            .map(|relation| Element::find_full(relation.origin_resource.id, pool).unwrap());
        Ok(elements.collect::<Vec<Element>>())
    }

    pub fn find_with_parents(
        id: Uuid,
        pool: &DbPool,
    ) -> Result<LandmarkWithParentsAndElements, PpdcError> {
        let origin_landmark = Landmark::find(id, pool)?;
        let mut elements: Vec<Element> = origin_landmark.find_elements(pool)?;
        let mut parents: Vec<Landmark> = vec![];
        let mut current_landmark = origin_landmark.clone();
        while let Some(parent) = current_landmark.find_parent(pool)? {
            let current_elements = parent.find_elements(pool)?;
            elements.extend(current_elements);
            parents.push(parent.clone());
            current_landmark = parent;
        }
        Ok(LandmarkWithParentsAndElements::new(
            origin_landmark,
            parents,
            elements,
        ))
    }

    pub fn get_analysis(self, pool: &DbPool) -> Result<Resource, PpdcError> {
        let resource_relations = ResourceRelation::find_target_for_resource(self.id, pool)?;
        let analysis = resource_relations
            .into_iter()
            .find(|relation| {
                relation.resource_relation.relation_type == "ownr".to_string()
                    && relation.target_resource.is_landscape_analysis()
            })
            .map(|relation| relation.target_resource)
            .ok_or(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Analysis not found".to_string(),
            ))?;
        Ok(analysis)
    }

    pub fn get_for_landscape_analysis(
        landscape_analysis_id: Uuid,
        relation_type: Option<&str>,
        pool: &DbPool,
    ) -> Result<Vec<Landmark>, PpdcError> {
        let resource_relations =
            ResourceRelation::find_origin_for_resource(landscape_analysis_id, pool)?;
        let landmarks = resource_relations
            .into_iter()
            .filter(|relation| {
                relation.origin_resource.is_landmark()
                    && relation_type.map_or(true, |relation_type| {
                        relation.resource_relation.relation_type == relation_type
                    })
            })
            .map(|relation| Landmark::from_resource(relation.origin_resource))
            .collect::<Vec<Landmark>>();
        Ok(landmarks)
    }
}

impl NewLandmark {
    pub fn to_new_resource(self) -> NewResource {
        NewResource {
            title: self.title,
            subtitle: self.subtitle,
            content: Some(self.content),
            external_content_url: None,
            comment: None,
            image_url: None,
            resource_type: Some(self.landmark_type.to_resource_type()),
            entity_type: Some(EntityType::Landmark),
            maturing_state: Some(self.maturing_state),
            publishing_state: Some(self.publishing_state),
            category_id: None,
            is_external: None,
        }
    }
}

impl From<Resource> for Landmark {
    fn from(resource: Resource) -> Self {
        Landmark::from_resource(resource)
    }
}
