use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;
use uuid::Uuid;
use crate::db;
use crate::schema::*;
use diesel::prelude::*;
use crate::entities::{error::{PpdcError}, user::User};
use crate::http::{HttpRequest, HttpResponse};

#[derive(Serialize, Deserialize, Queryable, Selectable)]
#[diesel(table_name=thought_input_usages)]
pub struct ThoughtInputUsage {
    id: Uuid,
    thought_input_id: Uuid,
    thought_output_id: Uuid,
    usage_reason: String,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

#[derive(Deserialize, Insertable, AsChangeset)]
#[diesel(table_name=thought_input_usages)]
pub struct NewThoughtInputUsage {
    thought_input_id: Uuid,
    thought_output_id: Uuid,
    usage_reason: String,
}

impl NewThoughtInputUsage {
    pub fn create(self) -> Result<ThoughtInputUsage, PpdcError> {
        let mut conn = db::establish_connection();

        let thought_input_usage = diesel::insert_into(thought_input_usages::table)
            .values(&self)
            .get_result(&mut conn)?;
        Ok(thought_input_usage)
    }
}

impl ThoughtInputUsage {
    pub fn find_for_thought_output(thought_output_id: Uuid) -> Result<Vec<ThoughtInputUsage>, PpdcError> {
        let mut conn = db::establish_connection();

        let thought_input_usages = thought_input_usages::table
            .filter(thought_input_usages::thought_output_id.eq(thought_output_id))
            .load::<ThoughtInputUsage>(&mut conn)?;
        Ok(thought_input_usages)
    }
}

pub fn post_thought_input_usage(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let thought_input_usage = serde_json::from_str::<NewThoughtInputUsage>(&request.body[..])?;
    HttpResponse::ok()
        .json(&thought_input_usage.create()?)
}

pub fn get_thought_input_usages_for_thought_output(thought_output_id: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let thought_output_id = HttpRequest::parse_uuid(thought_output_id)?;

    HttpResponse::ok()
        .json(&ThoughtInputUsage::find_for_thought_output(thought_output_id)?)
}
