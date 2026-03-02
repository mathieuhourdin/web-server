use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    resource::Resource,
    session::Session,
};
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationEntityPair {
    TraceMirrorToLandscapeAnalysis,
    TraceMirrorToTrace,
    LandscapeAnalysisToTrace,
    LandscapeAnalysisToLandscapeAnalysis,
    ElementToLandscapeAnalysis,
    ElementToTrace,
    ElementToTraceMirror,
    ElementToLandmark,
    ElementToElement,
    LandmarkToLandscapeAnalysis,
    LandmarkToLandmark,
    TraceToJournal,
    LensToLandscapeAnalysis,
    LensToTrace,
    TraceMirrorToLandmark,
    PublicPostToPublicPost,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationMeaning {
    AttachedToLandscape, // The origin analysis artifact belongs to / is attached to the target landscape analysis.
    Referenced,        // The origin references the target as contextual memory, without implying direct ownership.
    Mirrors,           // The origin trace mirror is the mirror representation of the target trace.
    Analyzes,          // The origin landscape analysis analyzes the target trace.
    ExtractedFrom,     // The origin element is extracted from the target trace content.
    ExtractedIn,       // The origin element is extracted/generated in the target trace mirror execution context.
    OwnedByAnalysis,   // The origin was produced/owned by the target analysis run.
    IncludesInScope,   // The origin lens includes the target analysis in its scope/history timeline.
    JournalItemOf,     // The origin trace entry belongs to the target journal.
    ThemeOf,           // The origin thematic element categorizes or groups the target unit element.
    AppliesTo,         // The origin normative/evaluative statement applies to the target objective element.
    HasCurrentHead,    // The origin lens has the target analysis as its current head/state pointer.
    HighLevelProjectRelatedTo, // The origin landmark is related/attached to the target high-level project landmark.
    Involves,          // The origin element involves the target landmark as part of its action/content.
    SubtaskOf,         // The origin task/transaction is a subtask of the target higher-level task/transaction.
    ReferenceMention,  // The origin trace mirror contains a reference mention mapped to the target landmark.
    TargetsTrace,      // The origin lens targets the target trace for processing.
    Bibliography,      // The origin cites the target as bibliography/source material.
    ReplayedFrom,      // The origin analysis is a replay/rerun from the target previous analysis.
    HasPrimaryLandmark, // The origin trace mirror has the target landmark as its primary landmark focus.
    HasPrimaryTheme,   // The origin trace mirror has the target landmark as its primary theme/topic.
    ForkedFrom,        // The origin lens is forked from the target landscape analysis snapshot.
    ChildOf,           // The origin is the child of the target in a parent-child hierarchy.
    Unknown,           // Unknown or not-yet-classified relation meaning.
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
