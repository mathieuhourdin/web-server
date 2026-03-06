use chrono::NaiveDateTime;
use serde::Serialize;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::analysis_summary::{AnalysisSummary, AnalysisSummaryType};
use crate::entities_v2::element::{Element, ElementRelationWithRelatedElement};
use crate::entities_v2::error::PpdcError;
use crate::entities_v2::landmark::Landmark;
use crate::entities_v2::landscape_analysis::model::LandscapeAnalysisType;
use crate::entities_v2::landscape_analysis::{LandscapeAnalysis, LandscapeAnalysisInputType};
use crate::entities_v2::reference::Reference;
use crate::entities_v2::trace_mirror::TraceMirror;
use crate::work_analyzer::analysis_context::AnalysisContext;

#[derive(Debug, Serialize)]
pub struct DaySummaryPromptContext {
    pub analysis_id: Uuid,
    pub user_id: Uuid,
    pub analysis_type: String,
    pub period_start: NaiveDateTime,
    pub period_end: NaiveDateTime,
    pub current_state_summary: String,
    pub existing_period_summary: Option<SummaryContextItem>,
    pub previous_day_summary: Option<PreviousDaySummaryContextItem>,
    pub user_traces: Vec<UserTraceContextItem>,
}

#[derive(Debug, Serialize)]
pub struct SummaryContextItem {
    pub title: String,
    pub short_content: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct PreviousDaySummaryContextItem {
    pub analysis_id: Uuid,
    pub period_start: NaiveDateTime,
    pub period_end: NaiveDateTime,
    pub summary: SummaryContextItem,
}

#[derive(Debug, Serialize)]
pub struct UserTraceContextItem {
    pub trace_mirror: TraceMirrorContextItem,
    pub references: Vec<ReferenceContextItem>,
    pub elements: Vec<ElementContextItem>,
    pub high_level_projects: Vec<HighLevelProjectContextItem>,
}

#[derive(Debug, Serialize)]
pub struct TraceMirrorContextItem {
    pub id: Uuid,
    pub trace_id: Uuid,
    pub trace_mirror_type: String,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub tags: Vec<String>,
    pub primary_landmark_id: Option<Uuid>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Clone)]
pub struct LandmarkBaseContextItem {
    pub id: Uuid,
    pub landmark_type: String,
    pub title: String,
    pub subtitle: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ReferenceContextItem {
    pub id: Uuid,
    pub tag_id: i32,
    pub span: String,
    pub reference_type: String,
    pub context_tags: Vec<String>,
    pub reference_variants: Vec<String>,
    pub landmark: Option<LandmarkBaseContextItem>,
}

#[derive(Debug, Serialize)]
pub struct ElementContextItem {
    pub id: Uuid,
    pub interaction_date: Option<NaiveDateTime>,
    pub element_type: String,
    pub element_subtype: String,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub extended_content: Option<String>,
    pub verb: String,
    pub landmark_tag_ids: Vec<i32>,
    pub landmarks: Vec<LandmarkBaseContextItem>,
    pub relations: Vec<ElementRelationContextItem>,
}

#[derive(Debug, Serialize)]
pub struct ElementRelationContextItem {
    pub relation_type: String,
    pub related_element_id: Uuid,
    pub related_element_title: String,
    pub related_element_subtitle: String,
}

#[derive(Debug, Serialize)]
pub struct HighLevelProjectContextItem {
    pub tag_id: i32,
    pub span: String,
    pub landmark: LandmarkBaseContextItem,
}

pub fn build(
    context: &AnalysisContext,
    analysis: &LandscapeAnalysis,
) -> Result<DaySummaryPromptContext, PpdcError> {
    let existing_period_summary = find_period_summary_for_analysis(analysis.id, &context.pool)?
        .map(|summary| SummaryContextItem {
            title: summary.title,
            short_content: summary.short_content,
            content: summary.content,
        });
    let previous_day_summary = find_previous_day_summary(analysis, &context.pool)?;

    let trace_mirror_ids = collect_trace_mirror_ids(analysis, &context.pool)?;
    let mut user_traces = Vec::new();
    for trace_mirror_id in trace_mirror_ids {
        let trace_mirror = TraceMirror::find_full_trace_mirror(trace_mirror_id, &context.pool)?;
        user_traces.push(build_user_trace_context(&trace_mirror, &context.pool)?);
    }

    Ok(DaySummaryPromptContext {
        analysis_id: analysis.id,
        user_id: analysis.user_id,
        analysis_type: analysis.landscape_analysis_type.to_db().to_string(),
        period_start: analysis.period_start,
        period_end: analysis.period_end,
        current_state_summary: analysis.plain_text_state_summary.clone(),
        existing_period_summary,
        previous_day_summary,
        user_traces,
    })
}

fn find_period_summary_for_analysis(
    analysis_id: Uuid,
    pool: &DbPool,
) -> Result<Option<AnalysisSummary>, PpdcError> {
    Ok(AnalysisSummary::find_for_analysis(analysis_id, pool)?
        .into_iter()
        .find(|summary| summary.summary_type == AnalysisSummaryType::PeriodRecap))
}

fn find_previous_day_summary(
    analysis: &LandscapeAnalysis,
    pool: &DbPool,
) -> Result<Option<PreviousDaySummaryContextItem>, PpdcError> {
    let scoped_lenses = analysis.get_scoped_lenses(pool)?;
    let current_lens = scoped_lenses
        .into_iter()
        .find(|lens| lens.current_landscape_id == Some(analysis.id));
    let Some(current_lens) = current_lens else {
        return Ok(None);
    };

    let previous_daily_analysis = current_lens
        .get_analysis_scope(pool)?
        .into_iter()
        .filter(|candidate| candidate.landscape_analysis_type == LandscapeAnalysisType::DailyRecap)
        .filter(|candidate| candidate.id != analysis.id)
        .filter(|candidate| candidate.period_end <= analysis.period_start)
        .max_by_key(|candidate| candidate.period_end);

    let Some(previous_daily_analysis) = previous_daily_analysis else {
        return Ok(None);
    };

    let Some(summary) = find_period_summary_for_analysis(previous_daily_analysis.id, pool)? else {
        return Ok(None);
    };

    Ok(Some(PreviousDaySummaryContextItem {
        analysis_id: previous_daily_analysis.id,
        period_start: previous_daily_analysis.period_start,
        period_end: previous_daily_analysis.period_end,
        summary: SummaryContextItem {
            title: summary.title,
            short_content: summary.short_content,
            content: summary.content,
        },
    }))
}

fn collect_trace_mirror_ids(
    analysis: &LandscapeAnalysis,
    pool: &DbPool,
) -> Result<Vec<Uuid>, PpdcError> {
    let mut trace_mirror_ids = analysis
        .get_inputs(pool)?
        .into_iter()
        .filter(|input| input.input_type == LandscapeAnalysisInputType::Covered)
        .filter_map(|input| input.trace_mirror_id)
        .collect::<Vec<_>>();

    if let Some(trace_mirror_id) = analysis.trace_mirror_id {
        trace_mirror_ids.push(trace_mirror_id);
    }

    trace_mirror_ids.sort();
    trace_mirror_ids.dedup();
    Ok(trace_mirror_ids)
}

fn build_user_trace_context(
    trace_mirror: &TraceMirror,
    pool: &DbPool,
) -> Result<UserTraceContextItem, PpdcError> {
    let references = Reference::find_for_trace_mirror(trace_mirror.id, pool)?;
    let mut high_level_projects = Vec::new();
    let references_context = references
        .iter()
        .map(|reference| {
            let landmark = reference
                .landmark_id
                .map(|landmark_id| Landmark::find(landmark_id, pool))
                .transpose()?
                .map(|landmark| landmark_to_base_context(&landmark));

            if let Some(landmark) = &landmark {
                if landmark.landmark_type == "high_level_project" {
                    high_level_projects.push(HighLevelProjectContextItem {
                        tag_id: reference.tag_id,
                        span: reference.mention.clone(),
                        landmark: landmark.clone(),
                    });
                }
            }

            Ok(ReferenceContextItem {
                id: reference.id,
                tag_id: reference.tag_id,
                span: reference.mention.clone(),
                reference_type: reference.reference_type.to_db().to_string(),
                context_tags: reference.context_tags.clone(),
                reference_variants: reference.reference_variants.clone(),
                landmark,
            })
        })
        .collect::<Result<Vec<_>, PpdcError>>()?;

    let elements_context = Element::find_for_trace(trace_mirror.trace_id, pool)?
        .into_iter()
        .filter(|element| element.trace_mirror_id == Some(trace_mirror.id))
        .map(|element| build_element_context(&element, &references, pool))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(UserTraceContextItem {
        trace_mirror: TraceMirrorContextItem {
            id: trace_mirror.id,
            trace_id: trace_mirror.trace_id,
            trace_mirror_type: trace_mirror.trace_mirror_type.to_db().to_string(),
            title: trace_mirror.title.clone(),
            subtitle: trace_mirror.subtitle.clone(),
            content: trace_mirror.content.clone(),
            tags: trace_mirror.tags.clone(),
            primary_landmark_id: trace_mirror.primary_resource_id,
            created_at: trace_mirror.created_at,
        },
        references: references_context,
        elements: elements_context,
        high_level_projects,
    })
}

fn build_element_context(
    element: &Element,
    references: &[Reference],
    pool: &DbPool,
) -> Result<ElementContextItem, PpdcError> {
    let landmarks = element
        .find_landmarks(pool)?
        .into_iter()
        .map(|landmark| landmark_to_base_context(&landmark))
        .collect::<Vec<_>>();
    let landmark_ids = landmarks
        .iter()
        .map(|landmark| landmark.id)
        .collect::<Vec<_>>();

    let mut landmark_tag_ids = references
        .iter()
        .filter(|reference| {
            reference
                .landmark_id
                .is_some_and(|landmark_id| landmark_ids.contains(&landmark_id))
        })
        .map(|reference| reference.tag_id)
        .collect::<Vec<_>>();
    landmark_tag_ids.sort();
    landmark_tag_ids.dedup();

    let relations = element
        .find_related_elements(pool)?
        .into_iter()
        .map(relation_to_context)
        .collect::<Vec<_>>();

    Ok(ElementContextItem {
        id: element.id,
        interaction_date: element.interaction_date,
        element_type: element.element_type.to_db().to_string(),
        element_subtype: element.element_subtype.to_db().to_string(),
        title: element.title.clone(),
        subtitle: element.subtitle.clone(),
        content: element.content.clone(),
        extended_content: element.extended_content.clone(),
        verb: element.verb.clone(),
        landmark_tag_ids,
        landmarks,
        relations,
    })
}

fn relation_to_context(relation: ElementRelationWithRelatedElement) -> ElementRelationContextItem {
    ElementRelationContextItem {
        relation_type: relation.relation_type,
        related_element_id: relation.related_element.id,
        related_element_title: relation.related_element.title,
        related_element_subtitle: relation.related_element.subtitle,
    }
}

fn landmark_to_base_context(landmark: &Landmark) -> LandmarkBaseContextItem {
    LandmarkBaseContextItem {
        id: landmark.id,
        landmark_type: landmark.landmark_type.to_code().to_string(),
        title: landmark.title.clone(),
        subtitle: landmark.subtitle.clone(),
        content: landmark.content.clone(),
    }
}
