use uuid::Uuid;
use chrono::NaiveDateTime;
use axum::{debug_handler, extract::{Extension, Json, Path}};
use crate::db::DbPool;
use crate::entities::error::{ErrorType, PpdcError};
use crate::entities::session::Session;
use crate::entities::{
    resource::{
        NewResource, 
        ResourceType, 
        MaturingState, 
        Resource
    },
    interaction::model::{
        NewInteraction,
        Interaction,
    },
    resource_relation::{
        ResourceRelation,
        NewResourceRelation,
    },
};
use serde::{Serialize, Deserialize};
use chrono::Utc;
use crate::entities_v2::{
    landscape_analysis::{self, LandscapeAnalysis, NewLandscapeAnalysis},
    trace::Trace
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Lens {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub name: String,
    pub description: String,
    pub fork_landscape_id: Option<Uuid>, // the landscape id of the forked lens. If None, starts from the beginning.
    pub current_landscape_id: Option<Uuid>, // the landscape id of the current state of the lens.
    pub current_trace_id: Uuid, // the trace id of the current state of the lens.
    pub current_state_date: NaiveDateTime, // the date of the current state of the lens.
    pub model_version: String, // the version of the model used to create the lens.
    pub autoplay: bool, // if true, the lens will be autoplayed. If not, you have to give a trace until which the lens will be played.
    pub is_primary: bool, // if true, the lens is the primary lens of the user.
    pub created_at: NaiveDateTime, // the date of creation of the lens.
    pub updated_at: NaiveDateTime, // the date of the last update of the lens.
}

pub struct NewLens {
    pub name: String,
    pub description: String,
    pub fork_landscape_id: Option<Uuid>,
    pub current_trace_id: Uuid,
    pub current_state_date: NaiveDateTime,
    pub current_landscape_id: Uuid,
    pub model_version: String,
    pub autoplay: bool,
    pub is_primary: bool,
    pub user_id: Uuid,
}

impl Lens {
    pub fn from_resource(resource: Resource) -> Lens {
        Lens {
            id: resource.id,
            user_id: None,
            name: resource.title,
            description: resource.subtitle,
            fork_landscape_id: None,
            current_landscape_id: None,
            current_trace_id: Uuid::nil(),
            current_state_date: NaiveDateTime::from_timestamp(0, 0),
            model_version: "".to_string(),
            autoplay: false,
            is_primary: false,
            created_at: resource.created_at,
            updated_at: resource.updated_at,
        }
    }
    pub fn to_resource(self) -> Resource {
        Resource {
            id: self.id,
            title: self.name,
            subtitle: self.description,
            content: "".to_string(),
            external_content_url: None,
            comment: None,
            image_url: None,
            resource_type: ResourceType::Lens,
            maturing_state: MaturingState::Draft,
            publishing_state: "drft".to_string(),
            category_id: None,
            is_external: false,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
    pub fn with_user_id(self, pool: &DbPool) -> Result<Lens, PpdcError> {
        let resource = self.clone().to_resource();
        let interaction = resource.find_resource_author_interaction(&pool)?;
        let user_id = interaction.interaction_user_id;
        let date = interaction.interaction_date;
        Ok(Lens {
            user_id: Some(user_id),
            current_state_date: date,
            ..self
        })
    }
    pub fn with_forked_landscape(self, pool: &DbPool) -> Result<Lens, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, &pool)?;
        let fork_landscape_id = targets.into_iter()
            .find(|target| target.resource_relation.relation_type == "fork".to_string())
            .map(|target| target.target_resource.id);
        Ok(Lens {
            fork_landscape_id: fork_landscape_id,
            ..self
        })
    }
    pub fn with_current_landscape(self, pool: &DbPool) -> Result<Lens, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, &pool)?;
        let current_landscape_id = targets.into_iter()
            .find(|target| target.resource_relation.relation_type == "head".to_string())
            .map(|target| target.target_resource.id);
        if current_landscape_id.is_none() {
            return Err(PpdcError::new(404, ErrorType::ApiError, "Current landscape not found".to_string()));
        }
        let current_landscape_id = current_landscape_id.unwrap();
        Ok(Lens {
            current_landscape_id: Some(current_landscape_id),
            ..self
        })
    }
    pub fn with_current_trace(self, pool: &DbPool) -> Result<Lens, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, &pool)?;
        let current_trace_id = targets.into_iter()
            .find(|target| target.resource_relation.relation_type == "trgt".to_string())
            .map(|target| target.target_resource.id);
        if current_trace_id.is_none() {
            return Err(PpdcError::new(404, ErrorType::ApiError, "Current trace not found".to_string()));
        }
        let current_trace_id = current_trace_id.unwrap();
        Ok(Lens {
            current_trace_id: current_trace_id,
            ..self
        })
    }
    pub fn find_full_lens(id: Uuid, pool: &DbPool) -> Result<Lens, PpdcError> {
        let lens = Resource::find(id, pool)?;
        let lens = Lens::from_resource(lens);
        let lens = lens.with_user_id(pool)?;
        let lens = lens.with_forked_landscape(pool)?;
        let lens = lens.clone().with_current_landscape(pool)?;
        let lens = lens.clone().with_current_trace(pool)?;
        Ok(lens)
    }
    pub fn delete(self, pool: &DbPool) -> Result<Lens, PpdcError> {
        let resource = self.to_resource();
        let deleted_resource = resource.delete(pool)?;
        Ok(Lens::from_resource(deleted_resource))
    }

    pub fn update_current_landscape(self, new_landscape_analysis_id: Uuid, pool: &DbPool) -> Result<Lens, PpdcError> {
        let current_landscape_relation = ResourceRelation::find_target_for_resource(self.id, pool)?;
        let current_landscape_relation = current_landscape_relation.into_iter()
            .find(|relation| relation.resource_relation.relation_type == "head".to_string())
            .map(|relation| relation.resource_relation);
        if current_landscape_relation.is_none() {
            return Err(PpdcError::new(404, ErrorType::ApiError, "Current landscape not found".to_string()));
        }
        let current_landscape_relation = current_landscape_relation.unwrap();
        current_landscape_relation.delete(pool)?;
        let mut new_current_landscape_relation = NewResourceRelation::new(self.id, new_landscape_analysis_id);
        new_current_landscape_relation.relation_type = Some("head".to_string());
        new_current_landscape_relation.user_id = Some(self.user_id.unwrap());
        new_current_landscape_relation.create(pool)?;
        Ok(self)
    }

    pub fn get_user_lenses(user_id: Uuid, pool: &DbPool) -> Result<Vec<Lens>, PpdcError> {
        let lenses = Interaction::find_paginated_outputs_for_user(0, 200, user_id, "lens", pool)?;
        let lenses = lenses.into_iter()
            .map(|interaction| Lens::from_resource(interaction.resource))
            .collect::<Vec<Lens>>();
        Ok(lenses)
    }
}

impl NewLens {
    pub fn new(payload: NewLensDto, current_trace_id: Uuid, current_landscape_id: Uuid, user_id: Uuid) -> NewLens {
        NewLens {
            name: payload.name.unwrap_or_default(),
            description: payload.description.unwrap_or_default(),
            fork_landscape_id: payload.fork_landscape_id,
            current_trace_id: current_trace_id,
            current_state_date: payload.current_state_date.unwrap_or_default(),
            current_landscape_id: current_landscape_id,
            model_version: payload.model_version.unwrap_or_default(),
            autoplay: payload.autoplay.unwrap_or_default(),
            is_primary: payload.is_primary.unwrap_or_default(),
            user_id: user_id,
        }
    }
    pub fn to_new_resource(self) -> NewResource {
        NewResource {
            title: self.name,
            subtitle: self.description,
            content: None,
            resource_type: Some(ResourceType::Lens),
            maturing_state: Some(MaturingState::Draft),
            publishing_state: Some("drft".to_string()),
            category_id: None,
            is_external: None,
            external_content_url: None,
            comment: None,
            image_url: None,
        }
    }
    pub fn create(self, pool: &DbPool) -> Result<Lens, PpdcError> {

        let user_id = self.user_id;
        let current_state_date = self.current_state_date;
        let current_trace_id = self.current_trace_id;
        let fork_landscape_id = self.fork_landscape_id;
        let current_landscape_id = self.current_landscape_id;

        let new_resource = self.to_new_resource();
        let created_resource = new_resource.create(pool)?;
        let mut new_interaction = NewInteraction::new(user_id, created_resource.id);
        new_interaction.interaction_type = Some("outp".to_string());
        new_interaction.interaction_date = Some(current_state_date);
        new_interaction.interaction_progress = 0;
        new_interaction.create(pool)?;
        if let Some(fork_landscape_id) = fork_landscape_id {
            let mut new_fork_relation = NewResourceRelation::new(created_resource.id, fork_landscape_id);
            new_fork_relation.relation_type = Some("fork".to_string());
            new_fork_relation.user_id = Some(user_id);
            new_fork_relation.create(pool)?;
        }
        let mut new_landscape_relation = NewResourceRelation::new(created_resource.id, current_landscape_id);
        new_landscape_relation.relation_type = Some("head".to_string());
        new_landscape_relation.user_id = Some(user_id);
        new_landscape_relation.create(pool)?;
        let mut new_trace_relation = NewResourceRelation::new(created_resource.id, current_trace_id);
        new_trace_relation.relation_type = Some("trgt".to_string());
        new_trace_relation.user_id = Some(user_id);
        new_trace_relation.create(pool)?;
        let lens = Lens::from_resource(created_resource);
        Ok(lens)
    }
}

#[derive(Deserialize)]
pub struct NewLensDto {
    pub name: Option<String>,
    pub description: Option<String>,
    pub fork_landscape_id: Option<Uuid>,
    pub current_trace_id: Uuid,
    pub current_state_date: Option<NaiveDateTime>,
    pub model_version: Option<String>,
    pub autoplay: Option<bool>,
    pub is_primary: Option<bool>,
}

#[debug_handler]
pub async fn post_lens_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewLensDto>,
) -> Result<Json<Lens>, PpdcError> {
    let start_landscape_id: Uuid;
    let traces: Vec<Trace>;
    if let Some(fork_landscape_id) = payload.fork_landscape_id {
        // we start analysis from a already existing landscape
        // just create new landscapes from this one
        start_landscape_id = fork_landscape_id;
        let fork_landscape = LandscapeAnalysis::find_full_analysis(fork_landscape_id, &pool)?;
        traces = Trace::get_between(session.user_id.unwrap(), fork_landscape.analyzed_trace_id.unwrap(), payload.current_trace_id, &pool)?;
    } else {
        // we start analysis from the first trace. We should first create a new landscape from user biography.
        // It will have no landmarks, but will have a global context from the biography.
        let initial_landscape = NewLandscapeAnalysis::new(
            "Analyse de la biographie".to_string(),
            "Analyse de la biographie".to_string(),
            "Analyse de la biographie".to_string(),
            session.user_id.unwrap(),
            Utc::now().naive_utc(),
            None,
            None,
        ).create(&pool)?;
        start_landscape_id = initial_landscape.id;
        traces = Trace::get_before(session.user_id.unwrap(), payload.current_trace_id, &pool)?;
    }
    let current_trace_id = traces.last().unwrap().id;
    let mut previous_landscape_analysis_id = start_landscape_id;
    let mut landscape_analysis_ids: Vec<Uuid> = Vec::new();
    for trace in traces {
        let new_landscape_analysis = NewLandscapeAnalysis::new_placeholder(
            session.user_id.unwrap(), 
            trace.id, 
            Some(previous_landscape_analysis_id)
        ).create(&pool)?;
        landscape_analysis_ids.push(new_landscape_analysis.id);
        previous_landscape_analysis_id = new_landscape_analysis.id;
    }

    // after we have the first landscape, we can create the landscape placeholders from the first one using traces as diff.
    let lens = NewLens::new(
        payload, current_trace_id, 
        previous_landscape_analysis_id, 
        session.user_id.unwrap()
    )
        .create(&pool)?;
    let lens = lens.with_user_id(&pool)?;
    let lens = lens.with_forked_landscape(&pool)?;
    let lens = lens.with_current_landscape(&pool)?;
    let lens = lens.with_current_trace(&pool)?;
    Ok(Json(lens))
}

#[debug_handler]
pub async fn delete_lens_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Lens>, PpdcError> {
    println!("Deleting lens: {:?}", id);
    let mut lens = Lens::find_full_lens(id, &pool)?;
    let mut landscape_analysis_id = lens.current_landscape_id;

    while let Some(id) = landscape_analysis_id {
        println!("Deleting landscape analysis: {:?}", id);
        let current_landscape_analysis = LandscapeAnalysis::find_full_analysis(id, &pool)?;
        println!("Current landscape analysis: {:?}", current_landscape_analysis);
        if let Some(parent_analysis_id) = current_landscape_analysis.parent_analysis_id {
            // update head to the parent landscape_analysis before to delete the current landscape_analysis
            println!("Updating head to parent landscape analysis: {:?}", parent_analysis_id);
            lens = lens.update_current_landscape(parent_analysis_id, &pool)?;
        } else {
            // we reached last landscape_analysis, delete the lens before deleting the root landscape_analysis
            // delete the lens first since we dont know where to put the head after deletion.
            println!("Deleting lens");
            lens = lens.delete(&pool)?;
            landscape_analysis::delete_leaf_and_cleanup(current_landscape_analysis.id, &pool)?;
            return Ok(Json(lens));
        }
        println!("Deleting landscape analysis: {:?}", current_landscape_analysis.id);
        let deletion_option = landscape_analysis::delete_leaf_and_cleanup(current_landscape_analysis.id, &pool)?;
        println!("Deletion option: {:?}", deletion_option);
        if let Some(_deletion_option) = deletion_option {
            // deletion succeded, continue with the parent.
            landscape_analysis_id = current_landscape_analysis.parent_analysis_id;

        } else {
            // we reached a landscape analysis referenced by other lens or landscape_analysis, just delete the lens
            println!("Reached a landscape analysis referenced by other lens or landscape_analysis, just deleting the lens");
            break;
        }
    }
    println!("Deleting lens: {:?}", id);
    let lens = lens.delete(&pool)?;
    println!("Lens deleted: {:?}", lens);
    Ok(Json(lens))
}

pub async fn get_user_lenses_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<Vec<Lens>>, PpdcError> {
    let lenses = Lens::get_user_lenses(session.user_id.unwrap(), &pool)?;
    Ok(Json(lenses))
}