use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Nullable, Text, Timestamp, Uuid as SqlUuid};
use uuid::Uuid;

use super::model::{Landmark, LandmarkType, LandmarkWithParentsAndElements};
use crate::db::DbPool;
use crate::entities::error::{ErrorType, PpdcError};
use crate::entities::resource::{
    entity_type::EntityType, maturing_state::MaturingState, resource_type::ResourceType, Resource,
};
use crate::entities_v2::analysis_orchestration::landscape_analysis::LandscapeAnalysis;
use crate::entities_v2::element::model::Element;

#[derive(QueryableByName)]
struct LandmarkRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    analysis_id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    user_id: Uuid,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    parent_id: Option<Uuid>,
    #[diesel(sql_type = Text)]
    title: String,
    #[diesel(sql_type = Text)]
    subtitle: String,
    #[diesel(sql_type = Text)]
    content: String,
    #[diesel(sql_type = Nullable<Text>)]
    external_content_url: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    comment: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    image_url: Option<String>,
    #[diesel(sql_type = Text)]
    landmark_type: String,
    #[diesel(sql_type = Text)]
    maturing_state: String,
    #[diesel(sql_type = Timestamp)]
    created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    updated_at: NaiveDateTime,
}

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

fn row_to_landmark(row: LandmarkRow) -> Landmark {
    Landmark {
        id: row.id,
        title: row.title,
        subtitle: row.subtitle,
        content: row.content,
        external_content_url: row.external_content_url,
        comment: row.comment,
        image_url: row.image_url,
        landmark_type: LandmarkType::from_code(&row.landmark_type).unwrap_or(LandmarkType::Resource),
        maturing_state: MaturingState::from_code(&row.maturing_state).unwrap_or(MaturingState::Draft),
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn load_landmark_row(id: Uuid, pool: &DbPool) -> Result<LandmarkRow, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");
    let mut rows = sql_query(
        r#"
        SELECT id, analysis_id, user_id, parent_id, title, subtitle, content, external_content_url,
               comment, image_url, landmark_type, maturing_state, created_at, updated_at
        FROM landmarks
        WHERE id = $1
        "#,
    )
    .bind::<SqlUuid, _>(id)
    .load::<LandmarkRow>(&mut conn)?;
    rows.pop().ok_or_else(|| {
        PpdcError::new(
            404,
            ErrorType::ApiError,
            "Landmark not found".to_string(),
        )
    })
}

impl Landmark {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Landmark, PpdcError> {
        Ok(row_to_landmark(load_landmark_row(id, pool)?))
    }

    pub fn find_parent(&self, pool: &DbPool) -> Result<Option<Landmark>, PpdcError> {
        let row = load_landmark_row(self.id, pool)?;
        if let Some(parent_id) = row.parent_id {
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
        let row = load_landmark_row(self.id, pool)?;
        let analysis = LandscapeAnalysis::find_full_analysis(row.analysis_id, pool)?;
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

        let mut query = crate::schema::landscape_landmarks::table
            .filter(
                crate::schema::landscape_landmarks::landscape_analysis_id.eq(landscape_analysis_id),
            )
            .into_boxed();
        if let Some(filter) = relation_filter {
            query = query.filter(crate::schema::landscape_landmarks::relation_type.eq(filter));
        }

        let landmark_ids = query
            .select(crate::schema::landscape_landmarks::landmark_id)
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
        let landmark_ids = crate::schema::landmarks::table
            .filter(crate::schema::landmarks::user_id.eq(user_id))
            .filter(crate::schema::landmarks::landmark_type.eq("HIGH_LEVEL_PROJECT"))
            .select(crate::schema::landmarks::id)
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
