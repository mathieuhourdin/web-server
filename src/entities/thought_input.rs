use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;
use uuid::Uuid;
use crate::db;
use crate::schema::*;
use diesel::prelude::*;
use crate::entities::{error::{PpdcError}, user::User};
use crate::http::{HttpRequest, HttpResponse};

#[derive(Serialize, Deserialize, Clone, Queryable, Selectable)]
#[diesel(table_name=thought_inputs)]
pub struct ThoughtInput {
    id: Uuid,
    resource_title: String,
    resource_author_name: String,
    resource_type: Option<String>,
    resource_link: Option<String>,
    resource_image_link: Option<String>,
    resource_comment: String,
    input_progress: i32,
    input_date: Option<NaiveDateTime>,
    input_comment: String,
    input_is_public: bool,
    input_user_id: Uuid,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime
}

#[derive(Deserialize, Insertable, AsChangeset)]
#[diesel(table_name=thought_inputs)]
pub struct NewThoughtInput {
    resource_title: String,
    resource_author_name: String,
    resource_type: Option<String>,
    resource_link: Option<String>,
    resource_image_link: Option<String>,
    resource_comment: String,
    input_progress: i32,
    input_date: Option<NaiveDateTime>,
    input_comment: String,
    input_is_public: bool,
    input_user_id: Option<Uuid>,
}

impl NewThoughtInput {
    pub fn create(self) -> Result<ThoughtInput, PpdcError> {
        let mut conn = db::establish_connection();

        let thought_input = diesel::insert_into(thought_inputs::table)
            .values(&self)
            .get_result(&mut conn)?;
        Ok(thought_input)
    }
}

impl ThoughtInput {
    pub fn find_paginated_public_by_user(offset: i64, limit: i64, user_id: Uuid) -> Result<Vec<ThoughtInput>, PpdcError> {
        let mut conn = db::establish_connection();

        let thought_input = thought_inputs::table
            .filter(thought_inputs::input_user_id.eq(user_id))
            .filter(thought_inputs::input_is_public.eq(true))
            .offset(offset)
            .limit(limit)
            .load::<ThoughtInput>(&mut conn)?;
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

pub fn post_thought_input_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {

    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }
    let mut thought_input = serde_json::from_str::<NewThoughtInput>(&request.body[..])?;
    thought_input.input_user_id = request.session.as_ref().unwrap().user_id;
    let thought_input = NewThoughtInput::create(thought_input)?;
    HttpResponse::ok()
        .json(&thought_input)
}
