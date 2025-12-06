use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;
use uuid::Uuid;
use crate::db::DbPool;
use crate::schema::*;
use diesel;
use diesel::prelude::*;
use crate::entities::{session::Session, error::{PpdcError}, user::User, interaction::model::{Interaction, NewInteraction}};
use resource_type::ResourceType;
use axum::{debug_handler, extract::{Query, Json, Path, Extension}};
use crate::pagination::PaginationParams;
use crate::entities::process_audio::qualify_user_input;
pub use maturing_state::MaturingState;

pub mod resource_type;
pub mod maturing_state;

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
        let mut conn = pool.get().expect("Failed to get a connection from the pool");

        let result = resources::table
            .filter(resources::id.eq(id))
            .first(&mut conn)?;
        Ok(result)
    }

    pub fn find_resource_author_interaction(self, pool: &DbPool) -> Result<Interaction, PpdcError> {
        let mut conn = pool.get().expect("Failed to get a connection from the pool");

        let result: Interaction = interactions::table
            .filter(interactions::interaction_type.eq("outp"))
            .filter(interactions::resource_id.eq(self.id))
            .first(&mut conn)?;
        Ok(result)
    }
}

impl NewResource {
    pub fn create(self, pool: &DbPool) -> Result<Resource, PpdcError> {
        let mut conn = pool.get().expect("Failed to get a connection from the pool");

        let result = diesel::insert_into(resources::table)
            .values(&self)
            .get_result(&mut conn)?;
        Ok(result)
    }
    
    pub fn update(self, id: &Uuid, pool: &DbPool) -> Result<Resource, PpdcError> {
        let mut conn = pool.get().expect("Failed to get a connection from the pool");

        let result = diesel::update(resources::table)
            .filter(resources::id.eq(id))
            .set(&self)
            .get_result(&mut conn)?;
        Ok(result)
    }

    async fn from_dto_qualified(dto: NewResourceDto) -> Self {
        let NewResourceDto {
            title,
            subtitle,
            content,
            external_content_url,
            comment,
            image_url,
            resource_type,
            maturing_state,
            publishing_state,
            category_id,
            is_external
        } = dto;
        let mut end_title;
        let mut end_subtitle;
        if (title.is_none() || title.as_ref().unwrap() == "" && !content.is_none() && content.as_ref().unwrap() != "") {
            let content = Some(content.clone());
            let resource_properties = qualify_user_input::extract_user_input_resource_info_with_gpt(content.unwrap().unwrap().as_str()).await.unwrap(); 
            end_title = resource_properties.title;
            end_subtitle = resource_properties.subtitle;
        } else {
            end_title = title.unwrap_or("".to_string());
            end_subtitle = subtitle.unwrap_or("".to_string());
        }
        Self {
            title: end_title,
            subtitle: end_subtitle,
            content,
            external_content_url,
            comment,
            image_url,
            resource_type,
            maturing_state: Some(MaturingState::Finished),
            publishing_state: Some("pbsh".to_string()),
            category_id,
            is_external
        }
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
        if author_id != session.user_id.unwrap()
        {
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
    Extension(session): Extension<Session>,
    Json(mut payload): Json<NewResourceDto>,
) -> Result<Json<Resource>, PpdcError> {
    let new_resource = NewResource::from_dto_qualified(payload).await;
    //new_resource.publishing_state = Some("drft".to_string());
    let resource = new_resource.create(&pool)?;
    let interaction = NewInteraction::create_instant(session.user_id.unwrap(), resource.id, &pool);

    Ok(Json(resource))
}

#[debug_handler]
pub async fn get_resource_author_interaction_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>
) -> Result<Json<Interaction>, PpdcError> {

    println!("get_resource_author_interaction_route");
    let resource = Resource::find(id, &pool)?;
    let author_interaction = resource.find_resource_author_interaction(&pool)?;
    Ok(Json(author_interaction))
}

#[debug_handler]
pub async fn get_resource_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>
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
