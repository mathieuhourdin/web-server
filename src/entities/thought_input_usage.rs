use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;
use uuid::Uuid;
use crate::db;
use crate::schema::*;
use diesel::prelude::*;
use crate::entities::{error::{PpdcError}, interaction::model::{Interaction, InteractionWithResource}, resource::Resource};
use crate::http::{HttpRequest, HttpResponse};

#[derive(Serialize, Deserialize, Queryable, Selectable)]
#[diesel(table_name=thought_input_usages)]
pub struct ThoughtInputUsage {
    id: Uuid,
    thought_input_id: Uuid,
    resource_id: Uuid,
    usage_reason: String,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

#[derive(Serialize, Queryable)]
pub struct ThoughtInputUsageWithThoughtInput {
    #[serde(flatten)]
    thought_input_usage: ThoughtInputUsage,
    thought_input: InteractionWithResource,
}

#[derive(Deserialize, Insertable, AsChangeset)]
#[diesel(table_name=thought_input_usages)]
pub struct NewThoughtInputUsage {
    thought_input_id: Uuid,
    resource_id: Uuid,
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
    pub fn find_for_resource(resource_id: Uuid) -> Result<Vec<ThoughtInputUsageWithThoughtInput>, PpdcError> {
        let mut conn = db::establish_connection();

        let thought_input_usages = thought_input_usages::table
            .inner_join(interactions::table.on(thought_input_usages::thought_input_id.eq(interactions::id)).inner_join(resources::table))
            .filter(thought_input_usages::resource_id.eq(resource_id))
            .select((ThoughtInputUsage::as_select(), Interaction::as_select(), Resource::as_select()))
            .load::<(ThoughtInputUsage, Interaction, Resource)>(&mut conn)?
            .into_iter()
            .map(|(thought_input_usage, thought_input, resource)| ThoughtInputUsageWithThoughtInput { thought_input_usage, thought_input: InteractionWithResource { interaction: thought_input, resource } })
            .collect();
        Ok(thought_input_usages)
    }
}

pub fn post_thought_input_usage_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let thought_input_usage = serde_json::from_str::<NewThoughtInputUsage>(&request.body[..])?;
    HttpResponse::ok()
        .json(&thought_input_usage.create()?)
}

pub fn get_thought_input_usages_for_resource_route(resource_id: &str, _request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let resource_id = HttpRequest::parse_uuid(resource_id)?;

    HttpResponse::ok()
        .json(&ThoughtInputUsage::find_for_resource(resource_id)?)
}
