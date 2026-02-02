use crate::entities_v2::{
    trace::Trace,
    element::{
        self,
        Element,
        NewElement,
        ElementType,
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



pub async fn run(trace: &Trace, user_id: Uuid, landscape_analysis_id: Uuid, landmarks: &Vec<Landmark>, pool: &DbPool) -> Result<Element, PpdcError> {
    let trace_header = header::extract_mirror_header(trace).await?;
    let trace_mirror = create_trace_mirror(trace, trace_header, user_id, landscape_analysis_id, pool).await?;
    let journal = Journal::find_full(trace.journal_id.unwrap(), pool)?;
    let journal_subtitle = journal.subtitle;
    if journal_subtitle == "note" {
        let primary_resource_suggestion = primary_resource::suggestion::extract(trace).await?;
        let primary_resource_matched = primary_resource::matching::run(primary_resource_suggestion, landmarks).await?;
        let trace_mirror_landmark_id: Uuid;
        if primary_resource_matched.candidate_id.is_some() {
            let element_landmark_id = primary_resource_matched.candidate_id.unwrap();
            trace_mirror_landmark_id = Uuid::parse_str(element_landmark_id.as_str())?;
        } else {
            let primary_resource_created = primary_resource::creation::run(primary_resource_matched, landscape_analysis_id, user_id, pool).await?;
            trace_mirror_landmark_id = primary_resource_created.id;
        }
        element::link_to_landmark(trace_mirror.id, trace_mirror_landmark_id, user_id, pool)?;
    }
    Ok(trace_mirror)
}

/// Creates a trace mirror element: runs MirrorHeader extraction, then creates an Element
/// with the extracted title/subtitle and the trace content.
pub async fn create_trace_mirror(trace: &Trace, header: MirrorHeader, user_id: Uuid, landscape_analysis_id: Uuid, pool: &DbPool) -> Result<Element, PpdcError> {
    let trace_mirror = NewElement::new(
        header.title,
        header.subtitle,
        trace.content.clone(),
        ElementType::TraceMirror,
        trace.id,
        None,
        landscape_analysis_id,
        user_id,
    );
    let trace_mirror = trace_mirror.create(pool)?;
    Ok(trace_mirror)
}