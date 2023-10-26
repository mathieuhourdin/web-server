use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;
use uuid::Uuid;
use crate::db;
use crate::schema::*;
use diesel::prelude::*;
use crate::entities::{error::{PpdcError}, interaction::model::{Interaction, InteractionWithResource}, resource::Resource};
use crate::http::{HttpRequest, HttpResponse};

#[derive(Serialize, Deserialize, Queryable, Selectable)]
#[diesel(table_name=resource_relations)]
pub struct ResourceRelation {
    id: Uuid,
    origin_resource_id: Uuid,
    target_resource_id: Uuid,
    relation_comment: String,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
    user_id: Uuid,
    relation_type: String,
}

#[derive(Serialize, Queryable)]
pub struct ResourceRelationWithOriginResource {
    #[serde(flatten)]
    resource_relation: ResourceRelation,
    origin_resource: Resource,
}

#[derive(Deserialize, Insertable, AsChangeset)]
#[diesel(table_name=resource_relations)]
pub struct NewResourceRelation {
    origin_resource_id: Uuid,
    target_resource_id: Uuid,
    relation_comment: String,
    user_id: Option<Uuid>,
    relation_type: Option<String>,
}

impl NewResourceRelation {
    pub fn create(self) -> Result<ResourceRelation, PpdcError> {
        let mut conn = db::establish_connection();

        let resource_relation = diesel::insert_into(resource_relations::table)
            .values(&self)
            .get_result(&mut conn)?;
        Ok(resource_relation)
    }
}

impl ResourceRelation {
    pub fn find_for_resource(target_resource_id: Uuid) -> Result<Vec<ResourceRelationWithOriginResource>, PpdcError> {
        let mut conn = db::establish_connection();

        let resource_relations = resource_relations::table
            .inner_join(resources::table.on(resource_relations::origin_resource_id.eq(resources::id)))
            .filter(resource_relations::target_resource_id.eq(target_resource_id))
            .select((ResourceRelation::as_select(), Resource::as_select()))
            .load::<(ResourceRelation, Resource)>(&mut conn)?
            .into_iter()
            .map(|(resource_relation, origin_resource)| ResourceRelationWithOriginResource { resource_relation, origin_resource })
            .collect();
        Ok(resource_relations)
    }
}

pub fn post_resource_relation_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }
    let mut resource_relation = serde_json::from_str::<NewResourceRelation>(&request.body[..])?;
    resource_relation.user_id = request.session.as_ref().unwrap().user_id;
    HttpResponse::ok()
        .json(&resource_relation.create()?)
}

pub fn get_resource_relations_for_resource_route(target_resource_id: &str, _request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let target_resource_id = HttpRequest::parse_uuid(target_resource_id)?;

    HttpResponse::ok()
        .json(&ResourceRelation::find_for_resource(target_resource_id)?)
}
