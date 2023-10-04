use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;
use uuid::Uuid;
use crate::db;
use crate::schema::*;
use diesel::prelude::*;
use crate::entities::{error::{PpdcError}, user::User};
use crate::http::{HttpRequest, HttpResponse};

#[derive(Serialize, Deserialize, Clone, Queryable, Selectable, AsChangeset)]
#[diesel(table_name=interactions)]
pub struct ThoughtInput {
    pub id: Uuid,
    pub resource_title: String,
    pub resource_subtitle: String,
    pub resource_content: String,
    pub resource_comment: String,
    pub interaction_user_id: Option<Uuid>,
    pub interaction_progress: i32,
    pub resource_maturing_state: String,
    pub resource_publishing_state: String,
    pub resource_parent_id: Option<Uuid>,
    pub resource_external_content_url: Option<String>,
    pub resource_image_url: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub resource_type: String,
    pub resource_category_id: Option<Uuid>,
    pub interaction_comment: Option<String>,
    pub interaction_date: Option<NaiveDateTime>,
    pub interaction_type: Option<String>,
    pub interaction_is_public: bool,
}

#[derive(Serialize, Queryable)]
pub struct ThoughtInputWithAuthor {
    #[serde(flatten)]
    pub thought_output: ThoughtInput,
    pub author: User,
}

#[derive(Deserialize, Insertable, Queryable, AsChangeset)]
#[diesel(table_name=interactions)]
pub struct NewThoughtInput {
    pub resource_title: String,
    pub resource_subtitle: Option<String>,
    pub resource_content: Option<String>,
    pub resource_comment: Option<String>,
    pub interaction_user_id: Option<Uuid>,
    pub interaction_progress: i32,
    pub resource_maturing_state: Option<String>,
    pub resource_publishing_state: Option<String>,
    pub resource_parent_id: Option<Uuid>,
    pub resource_external_content_url: Option<String>,
    pub resource_image_url: Option<String>,
    pub resource_type: String,
    pub resource_category_id: Option<Uuid>,
    pub interaction_comment: Option<String>,
    pub interaction_date: Option<NaiveDateTime>,
    pub interaction_type: Option<String>,
    pub interaction_is_public: bool,
}

impl NewThoughtInput {
    pub fn create(self) -> Result<ThoughtInput, PpdcError> {
        let mut conn = db::establish_connection();

        let thought_input = diesel::insert_into(interactions::table)
            .values(&self)
            .get_result(&mut conn)?;
        Ok(thought_input)
    }

    pub fn update(self, id: &Uuid) -> Result<ThoughtInput, PpdcError> {
        let mut conn = db::establish_connection();

        let thought_input = diesel::update(interactions::table)
            .filter(interactions::id.eq(id))
            .set(&self)
            .get_result(&mut conn)?;
        Ok(thought_input)
    }
}

impl ThoughtInput {

    pub fn find_paginated(offset: i64, limit: i64) -> Result<Vec<ThoughtInput>, PpdcError> {
        let mut conn = db::establish_connection();

        let thought_input = interactions::table
            .filter(interactions::interaction_type.eq("inpt"))
            .offset(offset)
            .limit(limit)
            .load::<ThoughtInput>(&mut conn)?;
        Ok(thought_input)
    }

    pub fn find_paginated_public_by_user(offset: i64, limit: i64, user_id: Uuid) -> Result<Vec<ThoughtInput>, PpdcError> {
        let mut conn = db::establish_connection();

        let thought_input = interactions::table
            .filter(interactions::interaction_type.eq("inpt"))
            .filter(interactions::interaction_user_id.eq(user_id))
            .filter(interactions::interaction_is_public.eq(true))
            .offset(offset)
            .limit(limit)
            .load::<ThoughtInput>(&mut conn)?;
        Ok(thought_input)
    }

    pub fn find(id: Uuid) -> Result<ThoughtInput, PpdcError> {
        let mut conn = db::establish_connection();

        let thought_input = interactions::table
            .filter(interactions::id.eq(id))
            .first(&mut conn)?;
        Ok(thought_input)
    }
}

pub fn get_thought_inputs_for_user(user_id: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let user_id = HttpRequest::parse_uuid(user_id)?;
    let limit: i64 = request.query.get("limit").unwrap_or(&"20".to_string()).parse().unwrap();
    let offset: i64 = request.query.get("offset").unwrap_or(&"0".to_string()).parse().unwrap();
    HttpResponse::ok()
        .json(&ThoughtInput::find_paginated_public_by_user(offset, limit, user_id)?)
}

pub fn get_thought_inputs(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let limit: i64 = request.query.get("limit").unwrap_or(&"20".to_string()).parse().unwrap();
    let offset: i64 = request.query.get("offset").unwrap_or(&"0".to_string()).parse().unwrap();
    HttpResponse::ok()
        .json(&ThoughtInput::find_paginated(offset, limit)?)
}

pub fn get_one_thought_input_route(id: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let id = HttpRequest::parse_uuid(id)?;
    HttpResponse::ok()
        .json(&ThoughtInput::find(id)?)
}

pub fn post_thought_input_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {

    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }
    let mut thought_input = serde_json::from_str::<NewThoughtInput>(&request.body[..])?;
    thought_input.interaction_user_id = request.session.as_ref().unwrap().user_id;
    thought_input.interaction_type = Some("inpt".to_string());
    thought_input.resource_maturing_state = Some("fnsh".to_string());
    thought_input.resource_publishing_state = Some("pbsh".to_string());
    let thought_input = NewThoughtInput::create(thought_input)?;
    HttpResponse::ok()
        .json(&thought_input)
}

pub fn put_thought_input_route(id: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }
    let id = HttpRequest::parse_uuid(id)?;
    let db_thought_input = ThoughtInput::find(id)?;
    let thought_input = serde_json::from_str::<NewThoughtInput>(&request.body[..])?;
    if db_thought_input.interaction_user_id.unwrap() != request.session.as_ref().unwrap().user_id.unwrap() 
    {
        return Ok(HttpResponse::unauthorized());
    }
    HttpResponse::ok()
        .json(&thought_input.update(&id)?)
}
