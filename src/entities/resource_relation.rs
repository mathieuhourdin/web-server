use crate::db::DbPool;
use crate::entities::resource::Resource;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::schema::*;
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
    fn typed(
        origin_resource_id: Uuid,
        target_resource_id: Uuid,
        relation_type: &str,
        relation_entity_pair: RelationEntityPair,
        relation_meaning: RelationMeaning,
    ) -> NewResourceRelation {
        Self {
            origin_resource_id,
            target_resource_id,
            relation_comment: "".to_string(),
            user_id: None,
            relation_type: Some(relation_type.to_string()),
            relation_entity_pair: Some(relation_entity_pair),
            relation_meaning: Some(relation_meaning),
        }
    }

    pub fn attached_to_landscape(
        trace_mirror_id: Uuid,
        landscape_analysis_id: Uuid,
    ) -> NewResourceRelation {
        Self::typed(
            trace_mirror_id,
            landscape_analysis_id,
            "lnds",
            RelationEntityPair::TraceMirrorToLandscapeAnalysis,
            RelationMeaning::AttachedToLandscape,
        )
    }

    pub fn mirrors_trace(trace_mirror_id: Uuid, trace_id: Uuid) -> NewResourceRelation {
        Self::typed(
            trace_mirror_id,
            trace_id,
            "trce",
            RelationEntityPair::TraceMirrorToTrace,
            RelationMeaning::Mirrors,
        )
    }

    pub fn has_primary_landmark(
        trace_mirror_id: Uuid,
        landmark_id: Uuid,
    ) -> NewResourceRelation {
        Self::typed(
            trace_mirror_id,
            landmark_id,
            "prir",
            RelationEntityPair::TraceMirrorToLandmark,
            RelationMeaning::HasPrimaryLandmark,
        )
    }

    pub fn has_primary_theme(trace_mirror_id: Uuid, theme_landmark_id: Uuid) -> NewResourceRelation {
        Self::typed(
            trace_mirror_id,
            theme_landmark_id,
            "prit",
            RelationEntityPair::TraceMirrorToLandmark,
            RelationMeaning::HasPrimaryTheme,
        )
    }

    pub fn reference_mention(trace_mirror_id: Uuid, landmark_id: Uuid) -> NewResourceRelation {
        Self::typed(
            trace_mirror_id,
            landmark_id,
            "rfrr",
            RelationEntityPair::TraceMirrorToLandmark,
            RelationMeaning::ReferenceMention,
        )
    }

    pub fn analyzes_trace(landscape_analysis_id: Uuid, trace_id: Uuid) -> NewResourceRelation {
        Self::typed(
            landscape_analysis_id,
            trace_id,
            "trce",
            RelationEntityPair::LandscapeAnalysisToTrace,
            RelationMeaning::Analyzes,
        )
    }

    pub fn child_of_landscape_analysis(
        child_landscape_analysis_id: Uuid,
        parent_landscape_analysis_id: Uuid,
    ) -> NewResourceRelation {
        Self::typed(
            child_landscape_analysis_id,
            parent_landscape_analysis_id,
            "prnt",
            RelationEntityPair::LandscapeAnalysisToLandscapeAnalysis,
            RelationMeaning::ChildOf,
        )
    }

    pub fn replayed_from_landscape_analysis(
        landscape_analysis_id: Uuid,
        previous_landscape_analysis_id: Uuid,
    ) -> NewResourceRelation {
        Self::typed(
            landscape_analysis_id,
            previous_landscape_analysis_id,
            "rply",
            RelationEntityPair::LandscapeAnalysisToLandscapeAnalysis,
            RelationMeaning::ReplayedFrom,
        )
    }

    pub fn referenced_landmark(
        landmark_id: Uuid,
        landscape_analysis_id: Uuid,
    ) -> NewResourceRelation {
        Self::typed(
            landmark_id,
            landscape_analysis_id,
            "refr",
            RelationEntityPair::LandmarkToLandscapeAnalysis,
            RelationMeaning::Referenced,
        )
    }

    pub fn owned_by_analysis_landmark(
        landmark_id: Uuid,
        landscape_analysis_id: Uuid,
    ) -> NewResourceRelation {
        Self::typed(
            landmark_id,
            landscape_analysis_id,
            "ownr",
            RelationEntityPair::LandmarkToLandscapeAnalysis,
            RelationMeaning::OwnedByAnalysis,
        )
    }

    pub fn child_of_landmark(child_landmark_id: Uuid, parent_landmark_id: Uuid) -> NewResourceRelation {
        Self::typed(
            child_landmark_id,
            parent_landmark_id,
            "prnt",
            RelationEntityPair::LandmarkToLandmark,
            RelationMeaning::ChildOf,
        )
    }

    pub fn high_level_project_related_to(
        landmark_id: Uuid,
        high_level_project_landmark_id: Uuid,
    ) -> NewResourceRelation {
        Self::typed(
            landmark_id,
            high_level_project_landmark_id,
            "hlpr",
            RelationEntityPair::LandmarkToLandmark,
            RelationMeaning::HighLevelProjectRelatedTo,
        )
    }

    pub fn owned_by_analysis_element(
        element_id: Uuid,
        landscape_analysis_id: Uuid,
    ) -> NewResourceRelation {
        Self::typed(
            element_id,
            landscape_analysis_id,
            "ownr",
            RelationEntityPair::ElementToLandscapeAnalysis,
            RelationMeaning::OwnedByAnalysis,
        )
    }

    pub fn extracted_from_trace(element_id: Uuid, trace_id: Uuid) -> NewResourceRelation {
        Self::typed(
            element_id,
            trace_id,
            "elmt",
            RelationEntityPair::ElementToTrace,
            RelationMeaning::ExtractedFrom,
        )
    }

    pub fn extracted_in_trace_mirror(
        element_id: Uuid,
        trace_mirror_id: Uuid,
    ) -> NewResourceRelation {
        Self::typed(
            element_id,
            trace_mirror_id,
            "trcm",
            RelationEntityPair::ElementToTraceMirror,
            RelationMeaning::ExtractedIn,
        )
    }

    pub fn involves_landmark(element_id: Uuid, landmark_id: Uuid) -> NewResourceRelation {
        Self::typed(
            element_id,
            landmark_id,
            "elmt",
            RelationEntityPair::ElementToLandmark,
            RelationMeaning::Involves,
        )
    }

    pub fn applies_to_element(origin_element_id: Uuid, target_element_id: Uuid) -> NewResourceRelation {
        Self::typed(
            origin_element_id,
            target_element_id,
            "applies_to",
            RelationEntityPair::ElementToElement,
            RelationMeaning::AppliesTo,
        )
    }

    pub fn theme_of_element(theme_element_id: Uuid, unit_element_id: Uuid) -> NewResourceRelation {
        Self::typed(
            theme_element_id,
            unit_element_id,
            "theme_of",
            RelationEntityPair::ElementToElement,
            RelationMeaning::ThemeOf,
        )
    }

    pub fn subtask_of_element(
        child_element_id: Uuid,
        parent_element_id: Uuid,
    ) -> NewResourceRelation {
        Self::typed(
            child_element_id,
            parent_element_id,
            "subtask_of",
            RelationEntityPair::ElementToElement,
            RelationMeaning::SubtaskOf,
        )
    }

    pub fn includes_in_scope(lens_id: Uuid, landscape_analysis_id: Uuid) -> NewResourceRelation {
        Self::typed(
            lens_id,
            landscape_analysis_id,
            "lnsa",
            RelationEntityPair::LensToLandscapeAnalysis,
            RelationMeaning::IncludesInScope,
        )
    }

    pub fn has_current_head(lens_id: Uuid, landscape_analysis_id: Uuid) -> NewResourceRelation {
        Self::typed(
            lens_id,
            landscape_analysis_id,
            "head",
            RelationEntityPair::LensToLandscapeAnalysis,
            RelationMeaning::HasCurrentHead,
        )
    }

    pub fn targets_trace(lens_id: Uuid, trace_id: Uuid) -> NewResourceRelation {
        Self::typed(
            lens_id,
            trace_id,
            "trgt",
            RelationEntityPair::LensToTrace,
            RelationMeaning::TargetsTrace,
        )
    }

    pub fn forked_from(lens_id: Uuid, landscape_analysis_id: Uuid) -> NewResourceRelation {
        Self::typed(
            lens_id,
            landscape_analysis_id,
            "fork",
            RelationEntityPair::LensToLandscapeAnalysis,
            RelationMeaning::ForkedFrom,
        )
    }

    pub fn journal_item_of(trace_id: Uuid, journal_id: Uuid) -> NewResourceRelation {
        Self::typed(
            trace_id,
            journal_id,
            "jrit",
            RelationEntityPair::TraceToJournal,
            RelationMeaning::JournalItemOf,
        )
    }

    pub fn bibliography(origin_post_id: Uuid, target_post_id: Uuid) -> NewResourceRelation {
        Self::typed(
            origin_post_id,
            target_post_id,
            "bibl",
            RelationEntityPair::PublicPostToPublicPost,
            RelationMeaning::Bibliography,
        )
    }

    pub fn create_attached_to_landscape(
        trace_mirror_id: Uuid,
        landscape_analysis_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::attached_to_landscape(trace_mirror_id, landscape_analysis_id).create_by(user_id, pool)
    }

    pub fn create_mirrors_trace(
        trace_mirror_id: Uuid,
        trace_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::mirrors_trace(trace_mirror_id, trace_id).create_by(user_id, pool)
    }

    pub fn create_has_primary_landmark(
        trace_mirror_id: Uuid,
        landmark_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::has_primary_landmark(trace_mirror_id, landmark_id).create_by(user_id, pool)
    }

    pub fn create_has_primary_theme(
        trace_mirror_id: Uuid,
        theme_landmark_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::has_primary_theme(trace_mirror_id, theme_landmark_id).create_by(user_id, pool)
    }

    pub fn create_reference_mention(
        trace_mirror_id: Uuid,
        landmark_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::reference_mention(trace_mirror_id, landmark_id).create_by(user_id, pool)
    }

    pub fn create_reference_mention_with_comment(
        trace_mirror_id: Uuid,
        landmark_id: Uuid,
        relation_comment: String,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        let mut relation = Self::reference_mention(trace_mirror_id, landmark_id);
        relation.relation_comment = relation_comment;
        relation.create_by(user_id, pool)
    }

    pub fn create_analyzes_trace(
        landscape_analysis_id: Uuid,
        trace_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::analyzes_trace(landscape_analysis_id, trace_id).create_by(user_id, pool)
    }

    pub fn create_child_of_landscape_analysis(
        child_landscape_analysis_id: Uuid,
        parent_landscape_analysis_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::child_of_landscape_analysis(child_landscape_analysis_id, parent_landscape_analysis_id)
            .create_by(user_id, pool)
    }

    pub fn create_replayed_from_landscape_analysis(
        landscape_analysis_id: Uuid,
        previous_landscape_analysis_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::replayed_from_landscape_analysis(landscape_analysis_id, previous_landscape_analysis_id)
            .create_by(user_id, pool)
    }

    pub fn create_referenced_landmark(
        landmark_id: Uuid,
        landscape_analysis_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::referenced_landmark(landmark_id, landscape_analysis_id).create_by(user_id, pool)
    }

    pub fn create_owned_by_analysis_landmark(
        landmark_id: Uuid,
        landscape_analysis_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::owned_by_analysis_landmark(landmark_id, landscape_analysis_id).create_by(user_id, pool)
    }

    pub fn create_child_of_landmark(
        child_landmark_id: Uuid,
        parent_landmark_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::child_of_landmark(child_landmark_id, parent_landmark_id).create_by(user_id, pool)
    }

    pub fn create_high_level_project_related_to(
        landmark_id: Uuid,
        high_level_project_landmark_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::high_level_project_related_to(landmark_id, high_level_project_landmark_id)
            .create_by(user_id, pool)
    }

    pub fn create_owned_by_analysis_element(
        element_id: Uuid,
        landscape_analysis_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::owned_by_analysis_element(element_id, landscape_analysis_id).create_by(user_id, pool)
    }

    pub fn create_extracted_from_trace(
        element_id: Uuid,
        trace_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::extracted_from_trace(element_id, trace_id).create_by(user_id, pool)
    }

    pub fn create_extracted_in_trace_mirror(
        element_id: Uuid,
        trace_mirror_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::extracted_in_trace_mirror(element_id, trace_mirror_id).create_by(user_id, pool)
    }

    pub fn create_involves_landmark(
        element_id: Uuid,
        landmark_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::involves_landmark(element_id, landmark_id).create_by(user_id, pool)
    }

    pub fn create_applies_to_element(
        origin_element_id: Uuid,
        target_element_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::applies_to_element(origin_element_id, target_element_id).create_by(user_id, pool)
    }

    pub fn create_theme_of_element(
        theme_element_id: Uuid,
        unit_element_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::theme_of_element(theme_element_id, unit_element_id).create_by(user_id, pool)
    }

    pub fn create_subtask_of_element(
        child_element_id: Uuid,
        parent_element_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::subtask_of_element(child_element_id, parent_element_id).create_by(user_id, pool)
    }

    pub fn create_includes_in_scope(
        lens_id: Uuid,
        landscape_analysis_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::includes_in_scope(lens_id, landscape_analysis_id).create_by(user_id, pool)
    }

    pub fn create_has_current_head(
        lens_id: Uuid,
        landscape_analysis_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::has_current_head(lens_id, landscape_analysis_id).create_by(user_id, pool)
    }

    pub fn create_targets_trace(
        lens_id: Uuid,
        trace_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::targets_trace(lens_id, trace_id).create_by(user_id, pool)
    }

    pub fn create_forked_from(
        lens_id: Uuid,
        landscape_analysis_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::forked_from(lens_id, landscape_analysis_id).create_by(user_id, pool)
    }

    pub fn create_journal_item_of(
        trace_id: Uuid,
        journal_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::journal_item_of(trace_id, journal_id).create_by(user_id, pool)
    }

    pub fn create_bibliography(
        origin_post_id: Uuid,
        target_post_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<ResourceRelation, PpdcError> {
        Self::bibliography(origin_post_id, target_post_id).create_by(user_id, pool)
    }

    pub fn create_by(mut self, user_id: Uuid, pool: &DbPool) -> Result<ResourceRelation, PpdcError> {
        self.user_id = Some(user_id);
        self.create(pool)
    }

    pub fn create(self, pool: &DbPool) -> Result<ResourceRelation, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let _ = self.relation_type.as_ref().ok_or_else(|| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                "relation_type must be set before creating resource relation".to_string(),
            )
        })?;
        let _ = self.relation_entity_pair.as_ref().ok_or_else(|| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                "relation_entity_pair must be set before creating resource relation".to_string(),
            )
        })?;
        let _ = self.relation_meaning.as_ref().ok_or_else(|| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                "relation_meaning must be set before creating resource relation".to_string(),
            )
        })?;

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
