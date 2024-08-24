use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;
use uuid::Uuid;
use crate::db;
use crate::schema::*;
use diesel;
use diesel::prelude::*;
use crate::entities::{error::{PpdcError}, user::User, interaction::model::Interaction};
use crate::http::{HttpRequest, HttpResponse};
use resource_type::ResourceType;
use axum::{debug_handler, extract::{Path, Extension, Query, Json}, http::StatusCode as AxumStatusCode, response::IntoResponse};
pub use maturing_state::MaturingState;

pub mod resource_type;
pub mod maturing_state;

#[derive(Serialize, Deserialize, Clone, Queryable, Selectable, AsChangeset)]
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
    pub category_id: Option<Uuid>,
    pub is_external: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
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
    pub category_id: Option<Uuid>,
    pub is_external: Option<bool>,
}

impl Resource {
    pub fn find(id: Uuid) -> Result<Resource, PpdcError> {
        let mut conn = db::establish_connection();

        let result = resources::table
            .filter(resources::id.eq(id))
            .first(&mut conn)?;
        Ok(result)
    }

    pub fn find_resource_author_interaction(self) -> Result<Interaction, PpdcError> {
        let mut conn = db::establish_connection();

        let result: Interaction = interactions::table
            .filter(interactions::interaction_type.eq("outp"))
            .filter(interactions::resource_id.eq(self.id))
            .first(&mut conn)?;
        Ok(result)
    }
}

impl NewResource {
    pub fn create(self) -> Result<Resource, PpdcError> {
        let mut conn = db::establish_connection();

        let result = diesel::insert_into(resources::table)
            .values(&self)
            .get_result(&mut conn)?;
        Ok(result)
    }
    
    pub fn update(self, id: &Uuid) -> Result<Resource, PpdcError> {
        let mut conn = db::establish_connection();

        let result = diesel::update(resources::table)
            .filter(resources::id.eq(id))
            .set(&self)
            .get_result(&mut conn)?;
        Ok(result)
    }
}

pub fn put_resource_route(id: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }
    let id = HttpRequest::parse_uuid(id)?;
    let db_resource = Resource::find(id)?;
    let resource = serde_json::from_str::<NewResource>(&request.body[..])?;
    let author_interaction = db_resource.find_resource_author_interaction();
    if author_interaction.is_ok() {
        let author_id = author_interaction.unwrap().interaction_user_id;
        if author_id != request.session.as_ref().unwrap().user_id.unwrap() 
        {
            let author_user = User::find(&author_id)?;
            if author_user.is_platform_user {
                return Ok(HttpResponse::unauthorized());
            }
        }
    }
    HttpResponse::ok()
        .json(&resource.update(&id)?)
}

pub fn post_resource_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }

    let mut resource = serde_json::from_str::<NewResource>(&request.body[..])?;
    resource.publishing_state = Some("drft".to_string());

    HttpResponse::ok().json(&resource.create()?)
}


pub fn get_resource_author_interaction_route(id: &str, _request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let id = HttpRequest::parse_uuid(id)?;
    let resource = Resource::find(id)?;
    let author_interaction = resource.find_resource_author_interaction().ok();
    HttpResponse::ok().json(&author_interaction)
}

pub fn get_resource_route(id: &str, _request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let id = HttpRequest::parse_uuid(id)?;
    HttpResponse::ok().json(&Resource::find(id)?)
}

#[derive(Deserialize)]
pub struct GetResourcesParams {
    is_external: Option<bool>,
    resource_type: Option<String>,
}

impl GetResourcesParams {
    pub fn is_external(&self) -> Option<bool> { 
        self.is_external
    }

    pub fn resource_type(&self) -> String {
        self.resource_type.clone().unwrap_or("all".into())
    }
}

#[debug_handler]
pub async fn get_resources_route(Query(filter_params): Query<GetResourcesParams>) -> Result<Json<Vec<Resource>>, PpdcError> {
    //let (offset, limit) = request.get_pagination();

    let mut conn = db::establish_connection();

    let mut query = resources::table
        //.offset(offset)
        //.limit(limit)
        .order(resources::updated_at.desc())
        .filter(resources::maturing_state.eq("fnsh"))
        .into_boxed();

    if let Some(is_external) = filter_params.is_external {
        query = query.filter(resources::is_external.eq(is_external));
    } 
    if filter_params.resource_type().as_str() == "pblm" {
        query = query.filter(resources::resource_type.eq("pblm"));
    }
    let results = query.load::<Resource>(&mut conn)?;

    Ok(Json(results))
}
