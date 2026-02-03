use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    resource::{
        entity_type::EntityType,
        maturing_state::MaturingState,
        resource_type::ResourceType,
        NewResource,
        Resource,
    },
    resource_relation::ResourceRelation,
};
use crate::entities_v2::{landmark::Landmark, lens::Lens};

use super::model::{LandscapeAnalysis, NewLandscapeAnalysis};

impl LandscapeAnalysis {
    /// Creates a LandscapeAnalysis from a Resource with default/placeholder values
    /// for fields that need to be hydrated from relations.
    pub fn from_resource(resource: Resource) -> LandscapeAnalysis {
        LandscapeAnalysis {
            id: resource.id,
            title: resource.title,
            subtitle: resource.subtitle,
            plain_text_state_summary: resource.content,
            interaction_date: None,
            user_id: Uuid::nil(),
            parent_analysis_id: None,
            analyzed_trace_id: None,
            processing_state: resource.maturing_state,
            created_at: resource.created_at,
            updated_at: resource.updated_at,
        }
    }

    /// Converts the LandscapeAnalysis back to a Resource.
    pub fn to_resource(&self) -> Resource {
        Resource {
            id: self.id,
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            content: self.plain_text_state_summary.clone(),
            external_content_url: None,
            comment: None,
            image_url: None,
            resource_type: ResourceType::Analysis,
            entity_type: EntityType::LandscapeAnalysis,
            maturing_state: self.processing_state,
            publishing_state: "drft".to_string(),
            category_id: None,
            is_external: false,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    /// Hydrates user_id and interaction_date from the author interaction.
    pub fn with_user_id(self, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        println!("Hydrating user_id for analysis: {:?}", self.id);
        let resource = self.to_resource();
        let interaction = resource.find_resource_author_interaction(pool)?;
        Ok(LandscapeAnalysis {
            user_id: interaction.interaction_user_id,
            interaction_date: Some(interaction.interaction_date),
            ..self
        })
    }

    /// Hydrates parent_analysis_id from resource relations.
    /// Looks for a relation of type "prnt" (parent) pointing to another analysis.
    pub fn with_parent_analysis(self, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        println!("Hydrating parent_analysis_id for analysis: {:?}", self.id);
        let targets = ResourceRelation::find_target_for_resource(self.id, pool)?;
        let parent_analysis_id = targets
            .into_iter()
            .find(|target| target.resource_relation.relation_type == "prnt")
            .map(|target| target.target_resource.id);
        Ok(LandscapeAnalysis { parent_analysis_id, ..self })
    }

    /// Hydrates trace_id from resource relations.
    /// Looks for a relation of type "trce" (trace) pointing to a trace resource.
    pub fn with_trace(self, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        println!("Hydrating trace_id for analysis: {:?}", self.id);
        let targets = ResourceRelation::find_target_for_resource(self.id, pool)?;
        let analyzed_trace_id = targets
            .into_iter()
            .find(|target| target.resource_relation.relation_type == "trce")
            .map(|target| target.target_resource.id);
        Ok(LandscapeAnalysis { analyzed_trace_id, ..self })
    }

    pub fn find_full_parent(&self, pool: &DbPool) -> Result<Option<LandscapeAnalysis>, PpdcError> {
        let parent_analysis_id = self.parent_analysis_id;
        if let Some(parent_analysis_id) = parent_analysis_id {
            let parent_analysis = LandscapeAnalysis::find_full_analysis(parent_analysis_id, pool)?;
            Ok(Some(parent_analysis))
        } else {
            Ok(None)
        }
    }

    pub fn find_all_parents(&self, pool: &DbPool) -> Result<Vec<LandscapeAnalysis>, PpdcError> {
        let mut parents = vec![];
        let mut current_analysis = self.clone();
        while let Some(parent_analysis) = current_analysis.find_full_parent(pool)? {
            parents.push(parent_analysis.clone());
            current_analysis = parent_analysis;
        }
        Ok(parents)
    }

    /// Finds a LandscapeAnalysis by id and fully hydrates it from the database.
    pub fn find_full_analysis(id: Uuid, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        println!("Finding full analysis: {:?}", id);
        let resource = Resource::find(id, pool)?;
        println!("Resource: {:?}", resource);
        let analysis = LandscapeAnalysis::from_resource(resource);
        let analysis = analysis.with_user_id(pool)?;
        let analysis = analysis.with_parent_analysis(pool)?;
        let analysis = analysis.with_trace(pool)?;
        Ok(analysis)
    }

    pub fn get_landmarks(&self, pool: &DbPool) -> Result<Vec<Landmark>, PpdcError> {
        let landmarks = Landmark::get_for_landscape_analysis(self.id, pool)?;
        Ok(landmarks)
    }

    pub fn get_children_landscape_analyses(&self, pool: &DbPool) -> Result<Vec<LandscapeAnalysis>, PpdcError> {
        let children_landscape_analyses = ResourceRelation::find_origin_for_resource(self.id, pool)?;
        let children_landscape_analyses = children_landscape_analyses
            .into_iter()
            .filter(|relation| {
                relation.resource_relation.relation_type == "prnt"
                && relation.origin_resource.is_landscape_analysis()
            })
            .map(|relation| LandscapeAnalysis::from_resource(relation.origin_resource))
            .collect::<Vec<LandscapeAnalysis>>();
        Ok(children_landscape_analyses)
    }

    pub fn get_heading_lens(&self, pool: &DbPool) -> Result<Vec<Lens>, PpdcError> {
        let lenses = ResourceRelation::find_origin_for_resource(self.id, pool)?;
        let lenses = lenses
            .into_iter()
            .filter(|relation| {
                relation.resource_relation.relation_type == "head"
                && relation.origin_resource.is_lens()
            })
            .map(|relation| Lens::from_resource(relation.origin_resource))
            .collect::<Vec<Lens>>();
        Ok(lenses)
    }
}

impl NewLandscapeAnalysis {
    /// Converts to a NewResource for database insertion.
    pub fn to_new_resource(&self) -> NewResource {
        NewResource {
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            content: Some(self.plain_text_state_summary.clone()),
            resource_type: Some(ResourceType::Analysis),
            entity_type: Some(EntityType::LandscapeAnalysis),
            maturing_state: Some(MaturingState::Draft),
            publishing_state: Some("drft".to_string()),
            category_id: None,
            is_external: Some(false),
            external_content_url: None,
            comment: None,
            image_url: None,
        }
    }
}

impl From<Resource> for LandscapeAnalysis {
    fn from(resource: Resource) -> Self {
        LandscapeAnalysis::from_resource(resource)
    }
}
