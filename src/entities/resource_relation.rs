use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;
use uuid::Uuid;
use crate::db;
use crate::schema::*;
use diesel::prelude::*;
use crate::entities::{session::Session, error::{PpdcError}, resource::Resource};
use axum::{debug_handler, extract::{Json, Path, Extension}};

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

#[derive(Serialize, Queryable)]
pub struct ResourceRelationWithTargetResource {
    #[serde(flatten)]
    resource_relation: ResourceRelation,
    target_resource: Resource,
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
    pub fn find_origin_for_resource(target_resource_id: Uuid) -> Result<Vec<ResourceRelationWithOriginResource>, PpdcError> {
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

    pub fn find_target_for_resource(origin_resource_id: Uuid) -> Result<Vec<ResourceRelationWithTargetResource>, PpdcError> {
        let mut conn = db::establish_connection();

        let resource_relations = resource_relations::table
            .inner_join(resources::table.on(resource_relations::target_resource_id.eq(resources::id)))
            .filter(resource_relations::origin_resource_id.eq(origin_resource_id))
            .select((ResourceRelation::as_select(), Resource::as_select()))
            .load::<(ResourceRelation, Resource)>(&mut conn)?
            .into_iter()
            .map(|(resource_relation, target_resource)| ResourceRelationWithTargetResource { resource_relation, target_resource })
            .collect();
        Ok(resource_relations)
    }
}

#[debug_handler]
pub async fn post_resource_relation_route(
    Extension(session): Extension<Session>,
    Json(mut payload): Json<NewResourceRelation>
) -> Result<Json<ResourceRelation>, PpdcError> {
    payload.user_id = session.user_id;
    Ok(Json(payload.create()?))
}

#[debug_handler]
pub async fn get_resource_relations_for_resource_route(
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ResourceRelationWithOriginResource>>, PpdcError> {
    Ok(Json(ResourceRelation::find_origin_for_resource(id)?))
}

#[debug_handler]
pub async fn get_targets_for_resource_route(
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ResourceRelationWithTargetResource>>, PpdcError> {

    Ok(Json(ResourceRelation::find_target_for_resource(id)?))
}
