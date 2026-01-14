use crate::db::DbPool;
use crate::entities::{
    error::PpdcError, interaction::model::Interaction, session::Session, user::User, landmark::Landmark,
};
use crate::pagination::PaginationParams;
use crate::schema::*;
use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use chrono::NaiveDateTime;
use diesel;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod maturing_state;
pub mod resource_type;
pub mod trace;

pub use maturing_state::MaturingState;
pub use resource_type::ResourceType;

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
    pub category_id: Option<Uuid>,
    pub is_external: Option<bool>,
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
            maturing_state: Some(MaturingState::Draft),
            publishing_state: Some("pbsh".to_string()),
            category_id: None,
            is_external: Some(false),
        }
    }

    pub fn create(self, pool: &DbPool) -> Result<Resource, PpdcError> {
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
            maturing_state: Some(resource.maturing_state),
            publishing_state: Some(resource.publishing_state),
            category_id: resource.category_id,
            is_external: Some(resource.is_external),
        }
    }
}

impl From<Landmark> for Resource {
    fn from(landmark: Landmark) -> Self {
        landmark.to_resource()
    }
}

#[debug_handler]
pub async fn put_resource_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<NewResource>,
) -> Result<Json<Resource>, PpdcError> {
    let db_resource = Resource::find(id, &pool)?;
    let author_interaction = db_resource.find_resource_author_interaction(&pool);
    if author_interaction.is_ok() {
        let author_id = author_interaction.unwrap().interaction_user_id;
        if author_id != session.user_id.unwrap() {
            let author_user = User::find(&author_id, &pool)?;
            if author_user.is_platform_user {
                return Err(PpdcError::unauthorized());
            }
        }
    }
    Ok(Json(payload.update(&id, &pool)?))
}

#[debug_handler]
pub async fn post_resource_route(
    Extension(pool): Extension<DbPool>,
    Json(payload): Json<NewResource>,
) -> Result<Json<Resource>, PpdcError> {
    Ok(Json(payload.create(&pool)?))
}

#[debug_handler]
pub async fn get_resource_author_interaction_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Interaction>, PpdcError> {
    println!("get_resource_author_interaction_route");
    let resource = Resource::find(id, &pool)?;
    let author_interaction = resource.find_resource_author_interaction(&pool)?;
    Ok(Json(author_interaction))
}

#[debug_handler]
pub async fn get_resource_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Resource>, PpdcError> {
    Ok(Json(Resource::find(id, &pool)?))
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
    Extension(pool): Extension<DbPool>,
    Query(filter_params): Query<GetResourcesParams>,
    Query(pagination): Query<PaginationParams>,
) -> Result<Json<Vec<Resource>>, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

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
    if filter_params.resource_type().as_str() == "miss" {
        query = query.filter(resources::resource_type.eq("miss"));
    }
    let results = query.load::<Resource>(&mut conn)?;

    Ok(Json(results))
}
