use crate::db::DbPool;
use crate::entities::error::PpdcError;
use crate::entities_v2::{landmark::Landmark, trace_mirror::TraceMirror};
use uuid::Uuid;

use super::{
    gpt_correction::correct_references_drafts,
    gpt_request::{
        build_context, build_high_level_project_index_map, build_high_level_projects_context,
        build_references_user_prompt, get_references_drafts_from_prompts,
        references_extraction_system_prompt,
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
    let trace_mirror_high_level_projects_references =
        trace_mirror.get_high_level_projects_references(pool)?;
    let high_level_projects_context =
        build_high_level_projects_context(&trace_mirror_high_level_projects_references, pool)?;
    let hlp_index_to_uuid = build_high_level_project_index_map(&high_level_projects_context);
    let initial_system_prompt = references_extraction_system_prompt();
    let initial_user_prompt = build_references_user_prompt(
        &trace_mirror.content,
        &context,
        &high_level_projects_context,
    )?;
    let extraction_result = get_references_drafts_from_prompts(
        initial_system_prompt.clone(),
        initial_user_prompt.clone(),
        analysis_id,
    )
    .await?;
    let extraction_result = correct_references_drafts(
        analysis_id,
        initial_system_prompt,
        initial_user_prompt,
        extraction_result,
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
