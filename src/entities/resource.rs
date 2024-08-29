use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;
use uuid::Uuid;
use crate::db::{self, DbPool};
use crate::schema::*;
use diesel;
use diesel::prelude::*;
use crate::entities::{session::Session, error::{PpdcError}, user::User, interaction::model::Interaction};
use resource_type::ResourceType;
use axum::{debug_handler, extract::{Query, Json, Path, Extension}};
use crate::pagination::PaginationParams;
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

#[debug_handler]
pub async fn put_resource_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<NewResource>,
) -> Result<Json<Resource>, PpdcError> {

    let db_resource = Resource::find(id)?;
    let author_interaction = db_resource.find_resource_author_interaction();
    if author_interaction.is_ok() {
        let author_id = author_interaction.unwrap().interaction_user_id;
        if author_id != session.user_id.unwrap()
        {
            let author_user = User::find(&author_id, &pool)?;
            if author_user.is_platform_user {
                return Err(PpdcError::unauthorized());
            }
        }
    }
    Ok(Json(payload.update(&id)?))
}

#[debug_handler]
pub async fn post_resource_route(
    Json(mut payload): Json<NewResource>,
) -> Result<Json<Resource>, PpdcError> {
    payload.publishing_state = Some("drft".to_string());

    Ok(Json(payload.create()?))
}

#[debug_handler]
pub async fn get_resource_author_interaction_route(
    Path(id): Path<Uuid>
) -> Result<Json<Interaction>, PpdcError> {

    println!("get_resource_author_interaction_route");
    let resource = Resource::find(id)?;
    let author_interaction = resource.find_resource_author_interaction()?;
    Ok(Json(author_interaction))
}

#[debug_handler]
pub async fn get_resource_route(
    Path(id): Path<Uuid>
) -> Result<Json<Resource>, PpdcError> {
    Ok(Json(Resource::find(id)?))
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
pub async fn get_resources_route(
    Query(filter_params): Query<GetResourcesParams>,
    Query(pagination): Query<PaginationParams>,
    Extension(pool): Extension<DbPool>,
) -> Result<Json<Vec<Resource>>, PpdcError> {

    let mut conn = pool.get().expect("Failed to get a connection from the pool");

    let mut query = resources::table
        .offset(pagination.offset())
        .limit(pagination.limit())
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
