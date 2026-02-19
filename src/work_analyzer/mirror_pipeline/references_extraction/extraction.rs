use crate::db::DbPool;
use crate::entities::error::PpdcError;
use crate::entities_v2::{landmark::Landmark, trace_mirror::TraceMirror};
use uuid::Uuid;

use super::{
    gpt_request::{
        build_context, build_high_level_project_index_map, build_high_level_projects_context,
        get_references_drafts,
    },
    persistence::persist_references_and_landmarks,
};

pub use super::gpt_request::{
    HighLevelProjectContextItem, IdentificationStatus, LandmarkReferenceContextItem, ReferenceContextItem,
    ReferencesExtractionResult, ReferencesExtractionResultItem,
};

pub async fn run(
    analysis_id: Uuid,
    trace_mirror_id: Uuid,
    landmarks: &Vec<Landmark>,
    pool: &DbPool,
) -> Result<TraceMirror, PpdcError> {
    let trace_mirror = TraceMirror::find_full_trace_mirror(trace_mirror_id, pool)?;
    let context = build_context(landmarks, pool)?;
    let trace_mirror_high_level_projects = trace_mirror.get_high_level_projects(pool)?;
    let high_level_projects_context =
        build_high_level_projects_context(&trace_mirror_high_level_projects);
    let hlp_index_to_uuid = build_high_level_project_index_map(&high_level_projects_context);
    let extraction_result = get_references_drafts(
        &trace_mirror.content,
        &context,
        &high_level_projects_context,
        analysis_id,
    )
    .await?;
    let ReferencesExtractionResult {
        references,
        tagged_text,
    } = extraction_result;

    persist_references_and_landmarks(
        analysis_id,
        &trace_mirror,
        landmarks,
        &context,
        &hlp_index_to_uuid,
        &references,
        pool,
    )?;

    let mut trace_mirror = trace_mirror;
    trace_mirror.content = tagged_text;
    trace_mirror.update(pool)
}
