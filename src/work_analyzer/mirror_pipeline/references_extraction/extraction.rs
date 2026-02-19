use crate::db::DbPool;
use crate::entities::error::PpdcError;
use crate::entities_v2::{landmark::Landmark, trace_mirror::TraceMirror};
use uuid::Uuid;

use super::{
    gpt_request::{build_context, get_references_drafts},
    persistence::persist_references_and_landmarks,
};

pub use super::gpt_request::{
    IdentificationStatus, LandmarkReferenceContextItem, ReferenceContextItem,
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
    let extraction_result =
        get_references_drafts(&trace_mirror.content, &context, analysis_id).await?;
    let ReferencesExtractionResult {
        references,
        tagged_text,
    } = extraction_result;

    persist_references_and_landmarks(
        analysis_id,
        &trace_mirror,
        landmarks,
        &context,
        &references,
        pool,
    )?;

    let mut trace_mirror = trace_mirror;
    trace_mirror.content = tagged_text;
    trace_mirror.update(pool)
}
