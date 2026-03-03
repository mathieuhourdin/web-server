use chrono::NaiveDateTime;
use diesel::prelude::*;
use uuid::Uuid;

use super::model::{Landmark, LandmarkType, LandmarkWithParentsAndElements};
use crate::db::DbPool;
use crate::entities::error::{ErrorType, PpdcError};
use crate::entities::resource::{
    entity_type::EntityType, maturing_state::MaturingState, resource_type::ResourceType, Resource,
};
use crate::entities_v2::analysis_orchestration::landscape_analysis::LandscapeAnalysis;
use crate::entities_v2::element::model::Element;
use crate::schema::{landmarks, landscape_landmarks};

type LandmarkTuple = (
    Uuid,
    Uuid,
    Option<Uuid>,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
    NaiveDateTime,
    NaiveDateTime,
);

impl LandmarkType {
    pub fn from_resource_type(resource_type: ResourceType) -> LandmarkType {
        match resource_type {
            ResourceType::Resource => LandmarkType::Resource,
            ResourceType::Topic => LandmarkType::Topic,
            ResourceType::Person => LandmarkType::Person,
            ResourceType::Mission => LandmarkType::Project,
            ResourceType::HighLevelProject => LandmarkType::HighLevelProject,
            ResourceType::Deliverable => LandmarkType::Deliverable,
            ResourceType::Question => LandmarkType::Question,
            _ => LandmarkType::Resource,
        }
    }

    pub fn to_resource_type(self) -> ResourceType {
        match self {
            LandmarkType::Resource => ResourceType::Resource,
            LandmarkType::Topic => ResourceType::Topic,
            LandmarkType::Person => ResourceType::Person,
            LandmarkType::Project => ResourceType::Mission,
            LandmarkType::HighLevelProject => ResourceType::HighLevelProject,
            LandmarkType::Deliverable => ResourceType::Deliverable,
            LandmarkType::Question => ResourceType::Question,
            LandmarkType::Organization
            | LandmarkType::Tool
            | LandmarkType::Habit
            | LandmarkType::Role
            | LandmarkType::Skill
            | LandmarkType::Place => ResourceType::Resource,
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
            publishing_state: "pbsh".to_string(),
            is_external: false,
            created_at: self.created_at,
            updated_at: self.updated_at,
            resource_subtype: None,
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
            created_at: resource.created_at,
            updated_at: resource.updated_at,
        }
    }
}

fn tuple_to_landmark(row: LandmarkTuple) -> Landmark {
    let (
        id,
        _analysis_id,
        _parent_id,
        title,
        subtitle,
        content,
        external_content_url,
        comment,
        image_url,
        landmark_type_raw,
        maturing_state_raw,
        created_at,
        updated_at,
    ) = row;

    Landmark {
        id,
        title,
        subtitle,
        content,
        external_content_url,
        comment,
        image_url,
        landmark_type: LandmarkType::from_code(&landmark_type_raw).unwrap_or(LandmarkType::Resource),
        maturing_state: MaturingState::from_code(&maturing_state_raw).unwrap_or(MaturingState::Draft),
        created_at,
        updated_at,
    }
}

fn select_landmark_columns() -> (
    landmarks::id,
    landmarks::analysis_id,
    landmarks::parent_id,
    landmarks::title,
    landmarks::subtitle,
    landmarks::content,
    landmarks::external_content_url,
    landmarks::comment,
    landmarks::image_url,
    landmarks::landmark_type,
    landmarks::maturing_state,
    landmarks::created_at,
    landmarks::updated_at,
) {
    (
        landmarks::id,
        landmarks::analysis_id,
        landmarks::parent_id,
        landmarks::title,
        landmarks::subtitle,
        landmarks::content,
        landmarks::external_content_url,
        landmarks::comment,
        landmarks::image_url,
        landmarks::landmark_type,
        landmarks::maturing_state,
        landmarks::created_at,
        landmarks::updated_at,
    )
}

impl Landmark {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Landmark, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let row = landmarks::table
            .filter(landmarks::id.eq(id))
            .select(select_landmark_columns())
            .first::<LandmarkTuple>(&mut conn)
            .optional()?;
        row.map(tuple_to_landmark).ok_or_else(|| {
            PpdcError::new(
                404,
                ErrorType::ApiError,
                "Landmark not found".to_string(),
            )
        })
    }

    pub fn find_parent(&self, pool: &DbPool) -> Result<Option<Landmark>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let parent_id = landmarks::table
            .filter(landmarks::id.eq(self.id))
            .select(landmarks::parent_id)
            .first::<Option<Uuid>>(&mut conn)?;
        if let Some(parent_id) = parent_id {
            return Landmark::find(parent_id, pool).map(Some);
        }
        Ok(None)
    }

    pub fn find_elements(&self, pool: &DbPool) -> Result<Vec<Element>, PpdcError> {
        Element::find_for_landmark(self.id, pool)
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
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let analysis_id = landmarks::table
            .filter(landmarks::id.eq(self.id))
            .select(landmarks::analysis_id)
            .first::<Uuid>(&mut conn)?;
        let analysis = LandscapeAnalysis::find_full_analysis(analysis_id, pool)?;
        Ok(Resource {
            id: analysis.id,
            title: analysis.title,
            subtitle: analysis.subtitle,
            content: analysis.plain_text_state_summary,
            external_content_url: None,
            comment: None,
            image_url: None,
            resource_type: ResourceType::Analysis,
            entity_type: EntityType::LandscapeAnalysis,
            maturing_state: analysis.processing_state.to_maturing_state(),
            publishing_state: "drft".to_string(),
            is_external: false,
            created_at: analysis.created_at,
            updated_at: analysis.updated_at,
            resource_subtype: None,
        })
    }

    pub fn get_for_landscape_analysis(
        landscape_analysis_id: Uuid,
        relation_type: Option<&str>,
        pool: &DbPool,
    ) -> Result<Vec<Landmark>, PpdcError> {
        let relation_filter = relation_type.map(|value| match value {
            "ownr" => "OWNED_BY_ANALYSIS",
            "refr" => "REFERENCED",
            other => other,
        });
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let mut query = landscape_landmarks::table
            .filter(landscape_landmarks::landscape_analysis_id.eq(landscape_analysis_id))
            .into_boxed();
        if let Some(filter) = relation_filter {
            query = query.filter(landscape_landmarks::relation_type.eq(filter));
        }

        let landmark_ids = query
            .select(landscape_landmarks::landmark_id)
            .load::<Uuid>(&mut conn)?;

        landmark_ids
            .into_iter()
            .map(|landmark_id| Landmark::find(landmark_id, pool))
            .collect::<Result<Vec<_>, _>>()
    }

    pub fn find_high_level_projects_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Landmark>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let landmark_ids = landmarks::table
            .filter(landmarks::user_id.eq(user_id))
            .filter(landmarks::landmark_type.eq("HIGH_LEVEL_PROJECT"))
            .select(landmarks::id)
            .load::<Uuid>(&mut conn)?;
        landmark_ids
            .into_iter()
            .map(|id| Landmark::find(id, pool))
            .collect()
    }
}

impl From<Resource> for Landmark {
    fn from(resource: Resource) -> Self {
        Landmark::from_resource(resource)
    }
}
