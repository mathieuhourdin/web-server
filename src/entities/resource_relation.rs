use crate::db::DbPool;
use crate::entities::{error::PpdcError, resource::Resource, session::Session};
use crate::schema::*;
use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Queryable, Selectable, Debug)]
#[diesel(table_name=resource_relations)]
pub struct ResourceRelation {
    id: Uuid,
    origin_resource_id: Uuid,
    target_resource_id: Uuid,
    relation_comment: String,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
    user_id: Uuid,
    pub relation_type: String,
}

#[derive(Serialize, Queryable, Debug)]
pub struct ResourceRelationWithOriginResource {
    #[serde(flatten)]
    pub resource_relation: ResourceRelation,
    pub origin_resource: Resource,
}

#[derive(Serialize, Queryable, Debug)]
pub struct ResourceRelationWithTargetResource {
    #[serde(flatten)]
    pub resource_relation: ResourceRelation,
    pub target_resource: Resource,
}

#[derive(Deserialize, Insertable, AsChangeset)]
#[diesel(table_name=resource_relations)]
pub struct NewResourceRelation {
    pub origin_resource_id: Uuid,
    pub target_resource_id: Uuid,
    pub relation_comment: String,
    pub user_id: Option<Uuid>,
    pub relation_type: Option<String>,
}

impl NewResourceRelation {
    pub fn new(origin_resource_id: Uuid, target_resource_id: Uuid) -> NewResourceRelation {
        Self {
            origin_resource_id,
            target_resource_id,
            relation_comment: "".to_string(),
            user_id: None,
            relation_type: None,
        }
    }

    pub fn create(self, pool: &DbPool) -> Result<ResourceRelation, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let resource_relation = diesel::insert_into(resource_relations::table)
            .values(&self)
            .get_result(&mut conn)?;
        Ok(resource_relation)
    }
}

impl ResourceRelation {
    pub fn find_origin_for_resource(
        target_resource_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<ResourceRelationWithOriginResource>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let resource_relations = resource_relations::table
            .inner_join(
                resources::table.on(resource_relations::origin_resource_id.eq(resources::id)),
            )
            .filter(resource_relations::target_resource_id.eq(target_resource_id))
            .select((ResourceRelation::as_select(), Resource::as_select()))
            .load::<(ResourceRelation, Resource)>(&mut conn)?
            .into_iter()
            .map(
                |(resource_relation, origin_resource)| ResourceRelationWithOriginResource {
                    resource_relation,
                    origin_resource,
                },
            )
            .collect();
        Ok(resource_relations)
    }

    pub fn find_target_for_resource(
        origin_resource_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<ResourceRelationWithTargetResource>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let resource_relations = resource_relations::table
            .inner_join(
                resources::table.on(resource_relations::target_resource_id.eq(resources::id)),
            )
            .filter(resource_relations::origin_resource_id.eq(origin_resource_id))
            .select((ResourceRelation::as_select(), Resource::as_select()))
            .load::<(ResourceRelation, Resource)>(&mut conn)?
            .into_iter()
            .map(
                |(resource_relation, target_resource)| ResourceRelationWithTargetResource {
                    resource_relation,
                    target_resource,
                },
            )
            .collect();
        Ok(resource_relations)
    }

    pub fn delete(self, pool: &DbPool) -> Result<ResourceRelation, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let resource_relation = diesel::delete(resource_relations::table)
            .filter(resource_relations::id.eq(self.id))
            .get_result(&mut conn)?;
        Ok(resource_relation)
    }
}

#[debug_handler]
pub async fn post_resource_relation_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(mut payload): Json<NewResourceRelation>,
) -> Result<Json<ResourceRelation>, PpdcError> {
    payload.user_id = session.user_id;
    Ok(Json(payload.create(&pool)?))
}

#[debug_handler]
pub async fn get_resource_relations_for_resource_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ResourceRelationWithOriginResource>>, PpdcError> {
    Ok(Json(ResourceRelation::find_origin_for_resource(id, &pool)?))
}

#[debug_handler]
pub async fn get_targets_for_resource_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ResourceRelationWithTargetResource>>, PpdcError> {
    Ok(Json(ResourceRelation::find_target_for_resource(id, &pool)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_target_for_resource() {
        use crate::environment::get_database_url;
        let database_url = get_database_url();
        let manager = diesel::r2d2::ConnectionManager::new(database_url);
        let pool = DbPool::new(manager).expect("Failed to create connection pool");
        let string_uuid = "744b085e-347f-410d-b395-48a4642f19d4";
        let uuid = Uuid::parse_str(string_uuid).unwrap();
        let resource_relations = ResourceRelation::find_target_for_resource(uuid, &pool).unwrap();
        println!("Resource relations: {:?}", resource_relations);
        assert_eq!(resource_relations.len(), 0);
    }
}
