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
use uuid::Uuid;
use crate::work_analyzer::trace_mirror::{
    header::{
        self,
        MirrorHeader,
    },
    primary_resource,
};
use crate::work_analyzer::analysis_processor::{AnalysisConfig, AnalysisContext, AnalysisInputs, AnalysisStateInitial, AnalysisStateMirror};

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



pub async fn run(_config: &AnalysisConfig, context: &AnalysisContext, inputs: &AnalysisInputs, state: &AnalysisStateInitial) -> Result<AnalysisStateMirror, PpdcError> {
    let log_header = context.log_header();
    let trace_header = header::extract_mirror_header(&inputs.trace, context.analysis_id).await?;
    let trace_mirror = create_trace_mirror(&inputs.trace, trace_header, context).await?;
    let journal = Journal::find_full(inputs.trace.journal_id.unwrap(), &context.pool)?;
    let journal_subtitle = journal.subtitle;
    if journal_subtitle == "note" {
        let primary_resource_suggestion = primary_resource::suggestion::extract(&inputs.trace, &log_header).await?;
        let primary_resource_matched =
            primary_resource::matching::run(primary_resource_suggestion, &inputs.previous_landscape_landmarks, &log_header).await?;
        let trace_mirror_landmark_id: Uuid;
        if primary_resource_matched.candidate_id.is_some() {
            let primary_resource_landmark_id = primary_resource_matched.candidate_id.unwrap();
            trace_mirror_landmark_id = Uuid::parse_str(primary_resource_landmark_id.as_str())?;
        } else {
            let primary_resource_created =
                primary_resource::creation::run(primary_resource_matched, context, &log_header).await?;
            trace_mirror_landmark_id = primary_resource_created.id;
        }
        trace_mirror::link_to_primary_resource(trace_mirror.id, trace_mirror_landmark_id, context.user_id, &context.pool)?;
    }
    Ok(AnalysisStateMirror {
        current_landscape: state.current_landscape.clone(),
        trace_mirror,
    })
}

/// Creates a trace mirror: runs MirrorHeader extraction, then creates a TraceMirror
/// with the extracted title/subtitle/tags.
pub async fn create_trace_mirror(trace: &Trace, header: MirrorHeader, context: &AnalysisContext) -> Result<TraceMirror, PpdcError> {
    let trace_mirror = NewTraceMirror::new(
        header.title,
        header.subtitle,
        trace.content.clone(),
        header.tags,
        trace.id,
        context.analysis_id,
        context.user_id,
        None, // primary_resource_id
        None, // primary_theme_id
        None, // interaction_date
    );
    let trace_mirror = trace_mirror.create(&context.pool)?;
    Ok(trace_mirror)
}
