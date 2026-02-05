use crate::entities_v2::{
    trace::Trace,
    trace_mirror::{
        self as trace_mirror,
        TraceMirror,
        NewTraceMirror,
    },
    journal::Journal,
    landmark::Landmark,
};
use crate::entities::error::PpdcError;
use crate::db::DbPool;
use uuid::Uuid;
use crate::work_analyzer::trace_mirror::{
    header::{
        self,
        MirrorHeader,
    },
    primary_resource,
};

// Here we have the pipeline step where we create a trace mirror element from the trace.
// Trace mirror has the content of the trace, and is linked to the trace.
// However it has more data
// title
// subtitle
// comment : a list of tags
// It knows what kind of trace we have.
// I wonder if the TraceMirror should be an element or a specific type of entity.


// First step : MirrorHeader
// This step only creates a title, a subtitle and a list of tags.

// Second step : MirrorQualification
// This step qualifies the trace against some questions : 
// - Is it a journal trace or a note trace ?
// Is it about a single resource or a collection of resources / themes ?
// I just remembered that this stage should just use the journal type for some part of it.
// I think we could make it easyer then.
// Still identify this step, but do nothing.

// Third step : PrimaryResourceLinking
// This step is run only if the trace is about a single resource.
// It may use multiple intermediary steps to link the trace to the resource.
// Try to identify the resource inside the trace first.
// Maybe some RAG one day ?
// Match the resource to an existing resource or create a new one.


pub struct NewPrimaryResource {
    pub resource_identifier: String,
    pub author: String,
    pub description: String,
    pub theme: Option<String>,
}



pub async fn run(trace: &Trace, user_id: Uuid, landscape_analysis_id: Uuid, landmarks: &Vec<Landmark>, pool: &DbPool) -> Result<TraceMirror, PpdcError> {
    let log_header = format!("analysis_id: {}", landscape_analysis_id);
    tracing::info!(
        target: "work_analyzer",
        "{} trace_mirror_start trace_id={} user_id={} landmarks={}",
        log_header,
        trace.id,
        user_id,
        landmarks.len()
    );
    let trace_header = header::extract_mirror_header(trace, &log_header).await?;
    tracing::info!(
        target: "work_analyzer",
        "{} trace_mirror_header_extracted title={} subtitle={}",
        log_header,
        trace_header.title,
        trace_header.subtitle
    );
    let trace_mirror = create_trace_mirror(trace, trace_header, user_id, landscape_analysis_id, pool).await?;
    tracing::info!(
        target: "work_analyzer",
        "{} trace_mirror_created trace_mirror_id={}",
        log_header,
        trace_mirror.id
    );
    let journal = Journal::find_full(trace.journal_id.unwrap(), pool)?;
    let journal_subtitle = journal.subtitle;
    if journal_subtitle == "note" {
        tracing::info!(
            target: "work_analyzer",
            "{} trace_mirror_primary_resource_start",
            log_header
        );
        let primary_resource_suggestion = primary_resource::suggestion::extract(trace, &log_header).await?;
        let primary_resource_matched =
            primary_resource::matching::run(primary_resource_suggestion, landmarks, &log_header).await?;
        let trace_mirror_landmark_id: Uuid;
        if primary_resource_matched.candidate_id.is_some() {
            let primary_resource_landmark_id = primary_resource_matched.candidate_id.unwrap();
            trace_mirror_landmark_id = Uuid::parse_str(primary_resource_landmark_id.as_str())?;
        } else {
            let primary_resource_created =
                primary_resource::creation::run(primary_resource_matched, landscape_analysis_id, user_id, pool, &log_header).await?;
            trace_mirror_landmark_id = primary_resource_created.id;
        }
        trace_mirror::link_to_primary_resource(trace_mirror.id, trace_mirror_landmark_id, user_id, pool)?;
        tracing::info!(
            target: "work_analyzer",
            "{} trace_mirror_primary_resource_linked primary_resource_id={}",
            log_header,
            trace_mirror_landmark_id
        );
    }
    tracing::info!(
        target: "work_analyzer",
        "{} trace_mirror_complete trace_mirror_id={}",
        log_header,
        trace_mirror.id
    );
    Ok(trace_mirror)
}

/// Creates a trace mirror: runs MirrorHeader extraction, then creates a TraceMirror
/// with the extracted title/subtitle/tags.
pub async fn create_trace_mirror(trace: &Trace, header: MirrorHeader, user_id: Uuid, landscape_analysis_id: Uuid, pool: &DbPool) -> Result<TraceMirror, PpdcError> {
    let trace_mirror = NewTraceMirror::new(
        header.title,
        header.subtitle,
        trace.content.clone(),
        header.tags,
        trace.id,
        landscape_analysis_id,
        user_id,
        None, // primary_resource_id
        None, // primary_theme_id
        None, // interaction_date
    );
    let trace_mirror = trace_mirror.create(pool)?;
    Ok(trace_mirror)
}
