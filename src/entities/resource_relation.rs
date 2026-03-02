use crate::db::DbPool;
use crate::entities::{
    error::{ErrorType, PpdcError},
    resource::Resource,
    session::Session,
};
use crate::schema::*;
use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use chrono::NaiveDateTime;
use diesel::deserialize::{self, FromSql};
use diesel::pg::{Pg, PgValue};
use diesel::prelude::*;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Text;
use diesel::{AsExpression, FromSqlRow};
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
    pub relation_entity_pair: RelationEntityPair,
    pub relation_meaning: RelationMeaning,
}

#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, AsExpression, FromSqlRow,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[diesel(sql_type = diesel::sql_types::Text)]
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

impl RelationEntityPair {
    pub fn to_code(&self) -> &'static str {
        match self {
            Self::TraceMirrorToLandscapeAnalysis => "TRACE_MIRROR_TO_LANDSCAPE_ANALYSIS",
            Self::TraceMirrorToTrace => "TRACE_MIRROR_TO_TRACE",
            Self::LandscapeAnalysisToTrace => "LANDSCAPE_ANALYSIS_TO_TRACE",
            Self::LandscapeAnalysisToLandscapeAnalysis => "LANDSCAPE_ANALYSIS_TO_LANDSCAPE_ANALYSIS",
            Self::ElementToLandscapeAnalysis => "ELEMENT_TO_LANDSCAPE_ANALYSIS",
            Self::ElementToTrace => "ELEMENT_TO_TRACE",
            Self::ElementToTraceMirror => "ELEMENT_TO_TRACE_MIRROR",
            Self::ElementToLandmark => "ELEMENT_TO_LANDMARK",
            Self::ElementToElement => "ELEMENT_TO_ELEMENT",
            Self::LandmarkToLandscapeAnalysis => "LANDMARK_TO_LANDSCAPE_ANALYSIS",
            Self::LandmarkToLandmark => "LANDMARK_TO_LANDMARK",
            Self::TraceToJournal => "TRACE_TO_JOURNAL",
            Self::LensToLandscapeAnalysis => "LENS_TO_LANDSCAPE_ANALYSIS",
            Self::LensToTrace => "LENS_TO_TRACE",
            Self::TraceMirrorToLandmark => "TRACE_MIRROR_TO_LANDMARK",
            Self::PublicPostToPublicPost => "PUBLIC_POST_TO_PUBLIC_POST",
            Self::Unknown => "UNKNOWN",
        }
    }

    pub fn from_code(code: &str) -> Result<Self, PpdcError> {
        match code {
            "TRACE_MIRROR_TO_LANDSCAPE_ANALYSIS" => Ok(Self::TraceMirrorToLandscapeAnalysis),
            "TRACE_MIRROR_TO_TRACE" => Ok(Self::TraceMirrorToTrace),
            "LANDSCAPE_ANALYSIS_TO_TRACE" => Ok(Self::LandscapeAnalysisToTrace),
            "LANDSCAPE_ANALYSIS_TO_LANDSCAPE_ANALYSIS" => {
                Ok(Self::LandscapeAnalysisToLandscapeAnalysis)
            }
            "ELEMENT_TO_LANDSCAPE_ANALYSIS" => Ok(Self::ElementToLandscapeAnalysis),
            "ELEMENT_TO_TRACE" => Ok(Self::ElementToTrace),
            "ELEMENT_TO_TRACE_MIRROR" => Ok(Self::ElementToTraceMirror),
            "ELEMENT_TO_LANDMARK" => Ok(Self::ElementToLandmark),
            "ELEMENT_TO_ELEMENT" => Ok(Self::ElementToElement),
            "LANDMARK_TO_LANDSCAPE_ANALYSIS" => Ok(Self::LandmarkToLandscapeAnalysis),
            "LANDMARK_TO_LANDMARK" => Ok(Self::LandmarkToLandmark),
            "TRACE_TO_JOURNAL" => Ok(Self::TraceToJournal),
            "LENS_TO_LANDSCAPE_ANALYSIS" => Ok(Self::LensToLandscapeAnalysis),
            "LENS_TO_TRACE" => Ok(Self::LensToTrace),
            "TRACE_MIRROR_TO_LANDMARK" => Ok(Self::TraceMirrorToLandmark),
            "PUBLIC_POST_TO_PUBLIC_POST" => Ok(Self::PublicPostToPublicPost),
            "UNKNOWN" => Ok(Self::Unknown),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Unknown relation_entity_pair code: {}", code),
            )),
        }
    }
}

impl ToSql<Text, Pg> for RelationEntityPair {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        <str as ToSql<Text, Pg>>::to_sql(self.to_code(), out)
    }
}

impl FromSql<Text, Pg> for RelationEntityPair {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        let s = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
        RelationEntityPair::from_code(&s)
            .map_err(|_| format!("Unknown relation_entity_pair code in DB: {}", s).into())
    }
}

#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, AsExpression, FromSqlRow,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[diesel(sql_type = diesel::sql_types::Text)]
pub enum RelationMeaning {
    AttachedToLandscape, // origin trace_mirror attaches to target landscape_analysis. Future DB: `trace_mirrors.landscape_analysis_id` FK.
    Referenced,        // origin landmark is referenced in target landscape_analysis context. Future DB: `landscape_landmarks` join table.
    Mirrors,           // origin trace_mirror mirrors target trace. Future DB: `trace_mirrors.trace_id` FK.
    Analyzes,          // origin landscape_analysis analyzes target trace. Future DB: `landscape_analyses.analyzed_trace_id` FK.
    ExtractedFrom,     // origin element is extracted from target trace. Future DB: `elements.trace_id` FK.
    ExtractedIn,       // origin element is extracted in target trace_mirror run. Future DB: `elements.trace_mirror_id` FK.
    OwnedByAnalysis,   // origin element/landmark is owned by target landscape_analysis. Future DB: `elements.analysis_id` and `landmarks.analysis_id` FKs.
    IncludesInScope,   // origin lens includes target landscape_analysis in scope. Future DB: `lens_analysis_scopes` join table.
    JournalItemOf,     // origin trace belongs to target journal. Future DB: `traces.journal_id` FK.
    ThemeOf,           // origin element (theme) is theme of target element (unit). Future DB: `element_relations` with typed edge.
    AppliesTo,         // origin element applies to target element. Future DB: `element_relations` with typed edge.
    HasCurrentHead,    // origin lens has target landscape_analysis as current head. Future DB: `lenses.current_landscape_analysis_id` FK.
    HighLevelProjectRelatedTo, // origin landmark relates to target high-level-project landmark. Future DB: `landmark_relations` with typed edge.
    Involves,          // origin element involves target landmark. Future DB: `element_landmarks` join table.
    SubtaskOf,         // origin element is subtask of target element. Future DB: `element_relations` with typed edge.
    ReferenceMention,  // origin trace_mirror mentions target landmark. Future DB: `references(trace_mirror_id, landmark_id, ...)`.
    TargetsTrace,      // origin lens targets target trace. Future DB: `lenses.target_trace_id` FK.
    Bibliography,      // origin public_post cites target public_post. Future DB: `post_relations` with typed edge.
    ReplayedFrom,      // origin landscape_analysis is replayed from target landscape_analysis. Future DB: `landscape_analyses.replayed_from_id` FK.
    HasPrimaryLandmark, // origin trace_mirror has target landmark as primary landmark. Future DB: `trace_mirrors.primary_landmark_id` FK.
    HasPrimaryTheme,   // origin trace_mirror has target landmark as primary theme. Future DB: `trace_mirrors.primary_theme_landmark_id` FK.
    ForkedFrom,        // origin lens is forked from target landscape_analysis. Future DB: `lenses.forked_from_landscape_analysis_id` FK.
    ChildOf,           // origin landmark/landscape_analysis is child of target parent. Future DB: `landmarks.parent_id` and `landscape_analyses.parent_id` FKs.
    Unknown,           // Unknown or not-yet-classified relation meaning.
}

impl RelationMeaning {
    pub fn to_code(&self) -> &'static str {
        match self {
            Self::AttachedToLandscape => "ATTACHED_TO_LANDSCAPE",
            Self::Referenced => "REFERENCED",
            Self::Mirrors => "MIRRORS",
            Self::Analyzes => "ANALYZES",
            Self::ExtractedFrom => "EXTRACTED_FROM",
            Self::ExtractedIn => "EXTRACTED_IN",
            Self::OwnedByAnalysis => "OWNED_BY_ANALYSIS",
            Self::IncludesInScope => "INCLUDES_IN_SCOPE",
            Self::JournalItemOf => "JOURNAL_ITEM_OF",
            Self::ThemeOf => "THEME_OF",
            Self::AppliesTo => "APPLIES_TO",
            Self::HasCurrentHead => "HAS_CURRENT_HEAD",
            Self::HighLevelProjectRelatedTo => "HIGH_LEVEL_PROJECT_RELATED_TO",
            Self::Involves => "INVOLVES",
            Self::SubtaskOf => "SUBTASK_OF",
            Self::ReferenceMention => "REFERENCE_MENTION",
            Self::TargetsTrace => "TARGETS_TRACE",
            Self::Bibliography => "BIBLIOGRAPHY",
            Self::ReplayedFrom => "REPLAYED_FROM",
            Self::HasPrimaryLandmark => "HAS_PRIMARY_LANDMARK",
            Self::HasPrimaryTheme => "HAS_PRIMARY_THEME",
            Self::ForkedFrom => "FORKED_FROM",
            Self::ChildOf => "CHILD_OF",
            Self::Unknown => "UNKNOWN",
        }
    }

    pub fn from_code(code: &str) -> Result<Self, PpdcError> {
        match code {
            "ATTACHED_TO_LANDSCAPE" => Ok(Self::AttachedToLandscape),
            "REFERENCED" => Ok(Self::Referenced),
            "MIRRORS" => Ok(Self::Mirrors),
            "ANALYZES" => Ok(Self::Analyzes),
            "EXTRACTED_FROM" => Ok(Self::ExtractedFrom),
            "EXTRACTED_IN" => Ok(Self::ExtractedIn),
            "OWNED_BY_ANALYSIS" => Ok(Self::OwnedByAnalysis),
            "INCLUDES_IN_SCOPE" => Ok(Self::IncludesInScope),
            "JOURNAL_ITEM_OF" => Ok(Self::JournalItemOf),
            "THEME_OF" => Ok(Self::ThemeOf),
            "APPLIES_TO" => Ok(Self::AppliesTo),
            "HAS_CURRENT_HEAD" => Ok(Self::HasCurrentHead),
            "HIGH_LEVEL_PROJECT_RELATED_TO" => Ok(Self::HighLevelProjectRelatedTo),
            "INVOLVES" => Ok(Self::Involves),
            "SUBTASK_OF" => Ok(Self::SubtaskOf),
            "REFERENCE_MENTION" => Ok(Self::ReferenceMention),
            "TARGETS_TRACE" => Ok(Self::TargetsTrace),
            "BIBLIOGRAPHY" => Ok(Self::Bibliography),
            "REPLAYED_FROM" => Ok(Self::ReplayedFrom),
            "HAS_PRIMARY_LANDMARK" => Ok(Self::HasPrimaryLandmark),
            "HAS_PRIMARY_THEME" => Ok(Self::HasPrimaryTheme),
            "FORKED_FROM" => Ok(Self::ForkedFrom),
            "CHILD_OF" => Ok(Self::ChildOf),
            "UNKNOWN" => Ok(Self::Unknown),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Unknown relation_meaning code: {}", code),
            )),
        }
    }
}

impl ToSql<Text, Pg> for RelationMeaning {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        <str as ToSql<Text, Pg>>::to_sql(self.to_code(), out)
    }
}

impl FromSql<Text, Pg> for RelationMeaning {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        let s = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
        RelationMeaning::from_code(&s)
            .map_err(|_| format!("Unknown relation_meaning code in DB: {}", s).into())
    }
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
    pub relation_entity_pair: Option<RelationEntityPair>,
    pub relation_meaning: Option<RelationMeaning>,
}

impl NewResourceRelation {
    pub fn new(origin_resource_id: Uuid, target_resource_id: Uuid) -> NewResourceRelation {
        Self {
            origin_resource_id,
            target_resource_id,
            relation_comment: "".to_string(),
            user_id: None,
            relation_type: None,
            relation_entity_pair: None,
            relation_meaning: None,
        }
    }

    pub fn create(mut self, pool: &DbPool) -> Result<ResourceRelation, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        if self.relation_type.is_none() {
            self.relation_type = Some("UNKNOWN".to_string());
        }
        if self.relation_entity_pair.is_none() {
            self.relation_entity_pair = Some(RelationEntityPair::Unknown);
        }
        if self.relation_meaning.is_none() {
            self.relation_meaning = Some(RelationMeaning::Unknown);
        }

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
