use crate::db::DbPool;
use crate::entities::{
    error::{ErrorType, PpdcError},
    interaction::model::{Interaction, InteractionWithResource, NewInteraction},
    resource::{maturing_state::MaturingState, resource_type::ResourceType, NewResource, Resource},
    resource_relation::{NewResourceRelation, ResourceRelation},
    session::Session,
};
use crate::entities_v2::{
    landmark::Landmark,
    lens::Lens,
};
use crate::work_analyzer::analysis_processor::run_analysis_pipeline;
use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use chrono::{Duration, NaiveDate, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LandscapeAnalysis {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub plain_text_state_summary: String,
    pub interaction_date: Option<NaiveDateTime>,
    pub user_id: Uuid,
    pub parent_analysis_id: Option<Uuid>,
    pub analyzed_trace_id: Option<Uuid>,
    pub processing_state: MaturingState,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

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
        Ok(LandscapeAnalysis {
            parent_analysis_id,
            ..self
        })
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
        Ok(LandscapeAnalysis {
            analyzed_trace_id: analyzed_trace_id,
            ..self
        })
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

    /// Updates the LandscapeAnalysis in the database.
    /// Only updates the underlying Resource fields (title, subtitle, content, maturing_state).
    pub fn update(self, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        let resource = self.to_resource();
        let updated_resource = NewResource::from(resource).update(&self.id, pool)?;
        Ok(LandscapeAnalysis {
            title: updated_resource.title,
            subtitle: updated_resource.subtitle,
            plain_text_state_summary: updated_resource.content,
            processing_state: updated_resource.maturing_state,
            updated_at: updated_resource.updated_at,
            ..self
        })
    }

    /// Updates the processing state of the analysis.
    pub fn set_processing_state(mut self, state: MaturingState, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        self.processing_state = state;
        self.update(pool)
    }

    pub fn get_landmarks(self, pool: &DbPool) -> Result<Vec<Landmark>, PpdcError> {
        let landmarks = Landmark::get_for_landscape_analysis(self.id, pool)?;
        Ok(landmarks)
    }

    pub fn get_children_landscape_analyses(&self, pool: &DbPool) -> Result<Vec<LandscapeAnalysis>, PpdcError> {
        let children_landscape_analyses = ResourceRelation::find_origin_for_resource(self.id, pool)?;
        let children_landscape_analyses = children_landscape_analyses
            .into_iter()
            .filter(|relation| {
                relation.resource_relation.relation_type == "prnt"
                && relation.origin_resource.resource_type == ResourceType::Analysis
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
                && relation.origin_resource.resource_type == ResourceType::Lens
            })
            .map(|relation| Lens::from_resource(relation.origin_resource))
            .collect::<Vec<Lens>>();
        Ok(lenses)
    }
    pub fn delete(self, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        let resource = self.to_resource();
        let deleted_resource = resource.delete(pool)?;
        Ok(LandscapeAnalysis::from_resource(deleted_resource))
    }
}

/// Struct for creating a new LandscapeAnalysis with all required data.
pub struct NewLandscapeAnalysis {
    pub title: String,
    pub subtitle: String,
    pub plain_text_state_summary: String,
    pub interaction_date: NaiveDateTime,
    pub user_id: Uuid,
    pub parent_analysis_id: Option<Uuid>,
    pub analyzed_trace_id: Option<Uuid>,
}

impl NewLandscapeAnalysis {
    /// Creates a new NewLandscapeAnalysis with all fields.
    pub fn new(
        title: String,
        subtitle: String,
        plain_text_state_summary: String,
        user_id: Uuid,
        interaction_date: NaiveDateTime,
        parent_analysis_id: Option<Uuid>,
        analyzed_trace_id: Option<Uuid>,
    ) -> NewLandscapeAnalysis {
        NewLandscapeAnalysis {
            title,
            subtitle,
            plain_text_state_summary,
            interaction_date,
            user_id,
            parent_analysis_id,
            analyzed_trace_id,
        }
    }

    pub fn new_placeholder(user_id: Uuid, trace_id: Uuid, parent_landscape_analysis_id: Option<Uuid>) -> NewLandscapeAnalysis {
        NewLandscapeAnalysis {
            title: format!("Analyse de la trace {}", trace_id),
            subtitle: "Analyse".to_string(),
            plain_text_state_summary: "Analyse".to_string(),
            interaction_date: Utc::now().naive_utc(),
            user_id,
            parent_analysis_id: parent_landscape_analysis_id,
            analyzed_trace_id: Some(trace_id),
        }
    }

    /// Converts to a NewResource for database insertion.
    fn to_new_resource(&self) -> NewResource {
        NewResource {
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            content: Some(self.plain_text_state_summary.clone()),
            resource_type: Some(ResourceType::Analysis),
            maturing_state: Some(MaturingState::Draft),
            publishing_state: Some("drft".to_string()),
            category_id: None,
            is_external: Some(false),
            external_content_url: None,
            comment: None,
            image_url: None,
        }
    }

    /// Creates the LandscapeAnalysis in the database.
    /// This creates the underlying Resource, Interaction, and ResourceRelations.
    pub fn create(self, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        let user_id = self.user_id;
        let interaction_date = self.interaction_date;
        let parent_analysis_id = self.parent_analysis_id;
        let analyzed_trace_id = self.analyzed_trace_id;

        // Create the underlying resource
        let new_resource = self.to_new_resource();
        let created_resource = new_resource.create(pool)?;

        // Create the author interaction
        let mut new_interaction = NewInteraction::new(user_id, created_resource.id);
        new_interaction.interaction_type = Some("outp".to_string());
        new_interaction.interaction_date = Some(interaction_date);
        new_interaction.interaction_progress = 0;
        new_interaction.create(pool)?;

        // Create parent analysis relation if provided
        if let Some(parent_id) = parent_analysis_id {
            let mut parent_relation = NewResourceRelation::new(created_resource.id, parent_id);
            parent_relation.relation_type = Some("prnt".to_string());
            parent_relation.user_id = Some(user_id);
            parent_relation.create(pool)?;
        }

        // Create analyzed trace relation if provided
        if let Some(trace_id) = analyzed_trace_id {
            let mut trace_relation = NewResourceRelation::new(created_resource.id, trace_id);
            trace_relation.relation_type = Some("trce".to_string());
            trace_relation.user_id = Some(user_id);
            trace_relation.create(pool)?;
        }

        // Return the fully hydrated analysis
        LandscapeAnalysis::find_full_analysis(created_resource.id, pool)
    }
}

#[derive(Deserialize)]
pub struct NewAnalysisDto {
    pub date: Option<NaiveDate>,
    pub user_id: Uuid,
}

#[debug_handler]
pub async fn post_analysis_route(
    Extension(pool): Extension<DbPool>,
    Extension(_session): Extension<Session>,
    Json(payload): Json<NewAnalysisDto>,
) -> Result<Json<Resource>, PpdcError> {
    let date = payload.date.unwrap_or_else(|| Utc::now().date_naive());
    let user_id = payload.user_id;

    // we find the last analysis for the user
    let last_analysis = find_last_analysis_resource(user_id, &pool)?;

    let analysis_title = match &last_analysis {
        Some(last_analysis) => {
            if last_analysis.interaction.interaction_date.clone()
                > (date - Duration::days(1)).and_hms_opt(12, 0, 0).expect("Date should be valid")
            {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "Analysis already exists for this day".to_string(),
                ));
            } else {
                format!(
                    "Analyse du {} au {}",
                    last_analysis
                        .interaction
                        .interaction_date
                        .clone()
                        .format("%Y-%m-%d")
                        .to_string(),
                    date.and_hms_opt(12, 0, 0)
                        .expect("Date should be valid")
                        .format("%Y-%m-%d")
                        .to_string()
                )
            }
        }
        None => {
            format!(
                "Première Analyse, jusqu'au {}",
                date.and_hms_opt(12, 0, 0)
                    .expect("Date should be valid")
                    .format("%Y-%m-%d")
                    .to_string()
            )
        }
    };
    // we create a new analysis resource that will be the support of all performed analysis in this analysis session
    let mut analysis_resource = NewResource::new(
        analysis_title,
        "Analyse".to_string(),
        "Analyse".to_string(),
        ResourceType::Analysis,
    );
    analysis_resource.maturing_state = Some(MaturingState::Draft);

    let analysis_resource = analysis_resource.create(&pool)?;

    // we create a new interaction for the analysis resource, with the given date
    let mut new_interaction = NewInteraction::new(user_id, analysis_resource.id);
    new_interaction.interaction_type = Some("outp".to_string());
    new_interaction.interaction_date = Some(date.and_hms_opt(12, 0, 0).unwrap());
    new_interaction.interaction_progress = 0;
    new_interaction.create(&pool)?;

    tokio::spawn(async move { run_analysis_pipeline(analysis_resource.id).await });

    //let resources = process_analysis(payload.user_id, date, &analysis_resource.id, last_analysis_id_option, &pool).await?;

    Ok(Json(analysis_resource))
}

#[debug_handler]
pub async fn delete_analysis_route(
    Extension(pool): Extension<DbPool>,
    Extension(_session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<LandscapeAnalysis>, PpdcError> {
    let analysis = delete_leaf_and_cleanup(id, &pool)?;
    Ok(Json(analysis.expect("No analysis found")))
}

#[debug_handler]
pub async fn get_landmarks_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Landmark>>, PpdcError> {
    let landscape = LandscapeAnalysis::find_full_analysis(id, &pool)?;
    let landmarks = landscape.get_landmarks(&pool)?;
    Ok(Json(landmarks))
}

#[debug_handler]    
pub async fn get_last_analysis_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<InteractionWithResource>, PpdcError> {
    println!("Getting last analysis for user: {:?}", session.user_id.unwrap());
    let last_analysis = find_last_analysis_resource(session.user_id.unwrap(), &pool)?;
    Ok(Json(last_analysis.expect("No last analysis found")))
}

pub fn delete_leaf_and_cleanup(
    id: Uuid,
    pool: &DbPool,
) -> Result<Option<LandscapeAnalysis>, PpdcError> {
    println!("Deleting leaf and cleanup for analysis: {:?}", id);
    let landscape_analysis = LandscapeAnalysis::find_full_analysis(id, &pool)?;
    println!("Landscape analysis: {:?}", landscape_analysis);
    let children_landscape_analyses = landscape_analysis.get_children_landscape_analyses(&pool)?;
    println!("Children landscape analyses: {:?}", children_landscape_analyses);
    let heading_lens = landscape_analysis.get_heading_lens(&pool)?;
    println!("Heading lens: {:?}", heading_lens);
    if children_landscape_analyses.len() > 0 || heading_lens.len() > 0 {
        println!("Children landscape analyses or heading lens found, not deleting");
        return Ok(None);
    }
    println!("Analysis: {:?}", landscape_analysis);
    let related_resources = ResourceRelation::find_origin_for_resource(id, &pool)?;
    println!("Related resources: {:?}", related_resources);
    for resource_relation in related_resources {
        println!("Resource relation: {:?}", resource_relation);
        // check if the resource is owned by the analysis (for elements and entities)
        if resource_relation.resource_relation.relation_type == "ownr".to_string() 
            && resource_relation.origin_resource.resource_type != ResourceType::Trace
            && resource_relation.origin_resource.resource_type != ResourceType::Lens
            && resource_relation.origin_resource.resource_type != ResourceType::Analysis
            {
            println!("Deleting resource: {:?}", resource_relation.origin_resource);
            resource_relation.origin_resource.delete(&pool)?;
        } else if resource_relation.origin_resource.resource_type == ResourceType::Trace {
            let mut trace = resource_relation.origin_resource;
            trace.maturing_state = MaturingState::Draft;
            trace.update(&pool)?;
        }
    }
    println!("Deleting landscape analysis: {:?}", landscape_analysis);
    let landscape_analysis = landscape_analysis.delete(&pool)?;
    Ok(Some(landscape_analysis))
}

pub fn find_last_analysis_resource(
    user_id: Uuid,
    pool: &DbPool,
) -> Result<Option<InteractionWithResource>, PpdcError> {
    let interaction = Interaction::find_last_analysis_for_user(user_id, pool)?;
    Ok(interaction)
}
