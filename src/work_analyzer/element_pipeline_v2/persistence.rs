use std::collections::{HashMap, HashSet, VecDeque};

use chrono::{Duration, NaiveDateTime};
use tracing::warn;
use uuid::Uuid;

use crate::entities::error::PpdcError;
use crate::entities_v2::{
    element::{link_to_landmark, Element, ElementSubtype, ElementType, NewElement},
    trace::Trace,
    trace_mirror::TraceMirror,
};
use crate::work_analyzer::analysis_processor::AnalysisContext;

use super::gpt_request::{
    DescriptiveKind, EvaluativeKind, GrammaticalExtractionOutput, NormativeForce, TransactionKind,
};

#[derive(Debug, Clone)]
struct ClaimNode {
    claim_id: String,
    title: String,
    subtitle: String,
    content: String,
    verb: String,
    interaction_date: Option<NaiveDateTime>,
    element_type: ElementType,
    element_subtype: ElementSubtype,
    direct_landmark_ids: HashSet<Uuid>,
    neighbors: HashSet<String>,
}

pub fn persist_extraction(
    context: &AnalysisContext,
    trace_mirror: &TraceMirror,
    extraction: &GrammaticalExtractionOutput,
    tag_to_landmark_id: &HashMap<i32, Uuid>,
) -> Result<Vec<Element>, PpdcError> {
    let default_interaction_date = find_trace_interaction_date(trace_mirror, context)?;
    let nodes = build_claim_nodes(extraction, tag_to_landmark_id, default_interaction_date)?;
    let effective_landmarks = compute_effective_landmark_ids(&nodes);
    persist_nodes(context, trace_mirror, nodes, effective_landmarks)
}

fn build_claim_nodes(
    extraction: &GrammaticalExtractionOutput,
    tag_to_landmark_id: &HashMap<i32, Uuid>,
    default_interaction_date: Option<NaiveDateTime>,
) -> Result<Vec<ClaimNode>, PpdcError> {
    let mut nodes = Vec::new();

    for claim in &extraction.transactions {
        let verb = claim.verb.trim();
        let target = claim.target.trim();
        let title = if target.is_empty() {
            format!("{}: {}", claim.id, verb)
        } else {
            format!("{}: {} {}", claim.id, verb, target)
        };
        let subtitle = format!("TRANSACTION / {}", claim.kind.as_str());
        let content = serde_json::to_string_pretty(claim)?;
        let mut neighbors = claim
            .related_transactions
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        if let Some(subtask_of) = &claim.subtask_of {
            neighbors.insert(subtask_of.clone());
        }

        nodes.push(ClaimNode {
            claim_id: claim.id.clone(),
            title,
            subtitle,
            content,
            verb: claim.verb.clone(),
            interaction_date: apply_date_offset(default_interaction_date, &claim.date_offset),
            element_type: ElementType::Transaction,
            element_subtype: map_transaction_kind(claim.kind),
            direct_landmark_ids: landmark_ids_from_tags(
                &claim.id,
                &claim.references_tags_id,
                tag_to_landmark_id,
            ),
            neighbors,
        });
    }

    for claim in &extraction.descriptives {
        let title = format!("{}: {}", claim.id, claim.object.trim());
        let subtitle = format!("DESCRIPTIVE / {}", claim.kind.as_str());
        let content = serde_json::to_string_pretty(claim)?;
        let neighbors = if claim.kind == DescriptiveKind::Theme {
            claim.unit_ids.iter().cloned().collect::<HashSet<_>>()
        } else {
            HashSet::new()
        };

        nodes.push(ClaimNode {
            claim_id: claim.id.clone(),
            title,
            subtitle,
            content,
            verb: "describe".to_string(),
            interaction_date: default_interaction_date,
            element_type: ElementType::Descriptive,
            element_subtype: map_descriptive_kind(claim.kind),
            direct_landmark_ids: landmark_ids_from_tags(
                &claim.id,
                &claim.references_tags_id,
                tag_to_landmark_id,
            ),
            neighbors,
        });
    }

    for claim in &extraction.normatives {
        let title = format!("{}: {}", claim.id, claim.force.as_str());
        let subtitle = format!("NORMATIVE / {}", claim.force.as_str());
        let content = serde_json::to_string_pretty(claim)?;
        let neighbors = HashSet::new();

        nodes.push(ClaimNode {
            claim_id: claim.id.clone(),
            title,
            subtitle,
            content,
            verb: "normative".to_string(),
            interaction_date: default_interaction_date,
            element_type: ElementType::Normative,
            element_subtype: map_normative_force(claim.force),
            direct_landmark_ids: HashSet::new(),
            neighbors,
        });
    }

    for claim in &extraction.evaluatives {
        let title = format!("{}: {} {}", claim.id, claim.kind.as_str(), claim.level);
        let subtitle = format!("EVALUATIVE / {}", claim.kind.as_str());
        let content = serde_json::to_string_pretty(claim)?;
        let neighbors = HashSet::new();

        nodes.push(ClaimNode {
            claim_id: claim.id.clone(),
            title,
            subtitle,
            content,
            verb: "evaluate".to_string(),
            interaction_date: default_interaction_date,
            element_type: ElementType::Evaluative,
            element_subtype: map_evaluative_kind(claim.kind),
            direct_landmark_ids: HashSet::new(),
            neighbors,
        });
    }

    Ok(nodes)
}

fn landmark_ids_from_tags(
    claim_id: &str,
    tags: &[i32],
    tag_to_landmark_id: &HashMap<i32, Uuid>,
) -> HashSet<Uuid> {
    let mut direct_landmark_ids = HashSet::new();
    for tag_id in tags {
        match tag_to_landmark_id.get(tag_id) {
            Some(landmark_id) => {
                direct_landmark_ids.insert(*landmark_id);
            }
            None => {
                warn!(
                    claim_id = claim_id,
                    tag_id = *tag_id,
                    "Skipping unknown reference tag id while mapping claim landmarks"
                );
            }
        }
    }
    direct_landmark_ids
}

fn compute_effective_landmark_ids(nodes: &[ClaimNode]) -> Vec<HashSet<Uuid>> {
    let mut adjacency = vec![Vec::<usize>::new(); nodes.len()];
    let id_to_index = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.claim_id.clone(), index))
        .collect::<HashMap<_, _>>();

    for (index, node) in nodes.iter().enumerate() {
        for neighbor_id in &node.neighbors {
            let Some(other_index) = id_to_index.get(neighbor_id) else {
                warn!(
                    claim_id = node.claim_id.as_str(),
                    unknown_neighbor_id = neighbor_id.as_str(),
                    "Skipping unknown claim id in relation edge"
                );
                continue;
            };

            if *other_index != index {
                adjacency[index].push(*other_index);
                adjacency[*other_index].push(index);
            }
        }
    }

    let mut visited = vec![false; nodes.len()];
    let mut effective = vec![HashSet::new(); nodes.len()];

    for start in 0..nodes.len() {
        if visited[start] {
            continue;
        }

        let mut component = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(start);
        visited[start] = true;

        while let Some(current) = queue.pop_front() {
            component.push(current);
            for neighbor in &adjacency[current] {
                if !visited[*neighbor] {
                    visited[*neighbor] = true;
                    queue.push_back(*neighbor);
                }
            }
        }

        let mut component_landmarks = HashSet::new();
        for index in &component {
            component_landmarks.extend(nodes[*index].direct_landmark_ids.iter().copied());
        }

        for index in component {
            effective[index] = component_landmarks.clone();
        }
    }

    effective
}

fn persist_nodes(
    context: &AnalysisContext,
    trace_mirror: &TraceMirror,
    nodes: Vec<ClaimNode>,
    effective_landmarks: Vec<HashSet<Uuid>>,
) -> Result<Vec<Element>, PpdcError> {
    let mut created = Vec::new();

    for (node, landmark_ids) in nodes.into_iter().zip(effective_landmarks) {
        let new_element = NewElement::new(
            node.title,
            node.subtitle,
            node.content,
            node.element_type,
            node.element_subtype,
            node.verb,
            node.interaction_date,
            trace_mirror.trace_id,
            Some(trace_mirror.id),
            None,
            context.analysis_id,
            context.user_id,
        );
        let element = new_element.create(&context.pool)?;

        for landmark_id in landmark_ids {
            link_to_landmark(element.id, landmark_id, context.user_id, &context.pool)?;
        }

        created.push(element);
    }

    Ok(created)
}

fn find_trace_interaction_date(
    trace_mirror: &TraceMirror,
    context: &AnalysisContext,
) -> Result<Option<NaiveDateTime>, PpdcError> {
    let trace = Trace::find_full_trace(trace_mirror.trace_id, &context.pool)?;
    Ok(trace.interaction_date)
}

fn apply_date_offset(
    base_interaction_date: Option<NaiveDateTime>,
    date_offset: &str,
) -> Option<NaiveDateTime> {
    let base = base_interaction_date?;
    let normalized = date_offset.trim().to_ascii_uppercase();

    match normalized.as_str() {
        "YESTERDAY" => Some(base - Duration::days(1)),
        "TWO_DAYS_AGO" => Some(base - Duration::days(2)),
        "TODAY_MORNING" => base.date().and_hms_opt(10, 0, 0),
        "TODAY_AFTERNOON" => base.date().and_hms_opt(15, 0, 0),
        "TOMORROW" => Some(base + Duration::days(1)),
        "LAST_WEEK" => Some(base - Duration::days(7)),
        "NEXT_WEEK" => Some(base + Duration::days(7)),
        _ => Some(base),
    }
}

fn map_transaction_kind(kind: TransactionKind) -> ElementSubtype {
    match kind {
        TransactionKind::Input => ElementSubtype::Input,
        TransactionKind::Output => ElementSubtype::Output,
        TransactionKind::Transformation => ElementSubtype::Transformation,
    }
}

fn map_descriptive_kind(kind: DescriptiveKind) -> ElementSubtype {
    match kind {
        DescriptiveKind::Unit => ElementSubtype::Unit,
        DescriptiveKind::Question => ElementSubtype::DescriptiveQuestion,
        DescriptiveKind::Theme => ElementSubtype::Theme,
    }
}

fn map_normative_force(force: NormativeForce) -> ElementSubtype {
    match force {
        NormativeForce::Plan => ElementSubtype::Plan,
        NormativeForce::Obligation => ElementSubtype::Obligation,
        NormativeForce::Recommendation => ElementSubtype::Recommendation,
        NormativeForce::Principle => ElementSubtype::Principle,
    }
}

fn map_evaluative_kind(kind: EvaluativeKind) -> ElementSubtype {
    match kind {
        EvaluativeKind::Emotion => ElementSubtype::Emotion,
        EvaluativeKind::Energy => ElementSubtype::Energy,
        EvaluativeKind::Quality => ElementSubtype::Quality,
        EvaluativeKind::Interest => ElementSubtype::Interest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::work_analyzer::element_pipeline_v2::gpt_request::GrammaticalExtractionOutput;
    use chrono::{NaiveDate, Timelike};

    fn sample_datetime() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 2, 18)
            .expect("valid date")
            .and_hms_opt(14, 30, 0)
            .expect("valid time")
    }

    fn sample_extraction_json() -> &'static str {
        r#"{
            "transactions": [
                {
                    "id": "tra_1",
                    "verb": "lire",
                    "kind": "INPUT",
                    "target": "High Output Management",
                    "theme": "management",
                    "status": "DONE",
                    "scope": "SUBTASK",
                    "life_domain": "WORK",
                    "date_offset": "YESTERDAY",
                    "subtask_of": null,
                    "related_transactions": [],
                    "spans": ["Hier j'ai lu High Output Management"],
                    "references_tags_id": [1]
                }
            ],
            "descriptives": [
                {
                    "id": "des_1",
                    "object": "Le management est un art d'execution",
                    "kind": "QUESTION",
                    "unit_ids": [],
                    "spans": ["Le management est un art d'execution ?"],
                    "references_tags_id": [1]
                }
            ],
            "normatives": [
                {
                    "id": "nor_1",
                    "force": "PLAN",
                    "polarity": "POSITIVE",
                    "applies_to": ["tra_1"],
                    "spans": ["Je dois continuer cette lecture"]
                }
            ],
            "evaluatives": [
                {
                    "id": "eva_1",
                    "kind": "INTEREST",
                    "polarity": "POSITIVE",
                    "level": 4,
                    "descriptive_ids": ["des_1"],
                    "transaction_ids": [],
                    "spans": ["C'est passionnant"]
                }
            ]
        }"#
    }

    #[test]
    fn parse_and_map_claims_to_element_types() {
        let extraction: GrammaticalExtractionOutput =
            serde_json::from_str(sample_extraction_json()).expect("sample extraction must parse");
        let landmark_id = Uuid::new_v4();
        let tag_to_landmark_id = HashMap::from([(1, landmark_id)]);

        let nodes = build_claim_nodes(&extraction, &tag_to_landmark_id, Some(sample_datetime()))
            .expect("build nodes");
        let by_id = nodes
            .iter()
            .map(|node| (node.claim_id.as_str(), node))
            .collect::<HashMap<_, _>>();

        let transaction = by_id.get("tra_1").expect("transaction node");
        assert_eq!(transaction.element_type, ElementType::Transaction);
        assert_eq!(transaction.element_subtype, ElementSubtype::Input);
        assert!(transaction.title.starts_with("tra_1:"));
        assert!(transaction.direct_landmark_ids.contains(&landmark_id));
        assert_eq!(
            transaction
                .interaction_date
                .expect("interaction date should be set")
                .date(),
            NaiveDate::from_ymd_opt(2026, 2, 17).expect("valid date")
        );

        let descriptive = by_id.get("des_1").expect("descriptive node");
        assert_eq!(descriptive.element_type, ElementType::Descriptive);
        assert_eq!(descriptive.element_subtype, ElementSubtype::DescriptiveQuestion);

        let normative = by_id.get("nor_1").expect("normative node");
        assert_eq!(normative.element_type, ElementType::Normative);
        assert_eq!(normative.element_subtype, ElementSubtype::Plan);

        let evaluative = by_id.get("eva_1").expect("evaluative node");
        assert_eq!(evaluative.element_type, ElementType::Evaluative);
        assert_eq!(evaluative.element_subtype, ElementSubtype::Interest);
    }

    #[test]
    fn connected_component_propagates_landmarks_transitively() {
        let landmark_id = Uuid::new_v4();
        let node_a = ClaimNode {
            claim_id: "tra_1".to_string(),
            title: "".to_string(),
            subtitle: "".to_string(),
            content: "".to_string(),
            verb: "".to_string(),
            interaction_date: None,
            element_type: ElementType::Transaction,
            element_subtype: ElementSubtype::Input,
            direct_landmark_ids: HashSet::from([landmark_id]),
            neighbors: HashSet::from(["tra_2".to_string()]),
        };
        let node_b = ClaimNode {
            claim_id: "tra_2".to_string(),
            title: "".to_string(),
            subtitle: "".to_string(),
            content: "".to_string(),
            verb: "".to_string(),
            interaction_date: None,
            element_type: ElementType::Transaction,
            element_subtype: ElementSubtype::Output,
            direct_landmark_ids: HashSet::new(),
            neighbors: HashSet::from(["tra_3".to_string()]),
        };
        let node_c = ClaimNode {
            claim_id: "tra_3".to_string(),
            title: "".to_string(),
            subtitle: "".to_string(),
            content: "".to_string(),
            verb: "".to_string(),
            interaction_date: None,
            element_type: ElementType::Transaction,
            element_subtype: ElementSubtype::Output,
            direct_landmark_ids: HashSet::new(),
            neighbors: HashSet::new(),
        };

        let effective = compute_effective_landmark_ids(&[node_a, node_b, node_c]);
        assert!(effective[0].contains(&landmark_id));
        assert!(effective[1].contains(&landmark_id));
        assert!(effective[2].contains(&landmark_id));
    }

    #[test]
    fn unknown_reference_tags_are_skipped_without_panic() {
        let extraction: GrammaticalExtractionOutput =
            serde_json::from_str(sample_extraction_json()).expect("sample extraction must parse");
        let tag_to_landmark_id = HashMap::new();
        let nodes = build_claim_nodes(&extraction, &tag_to_landmark_id, Some(sample_datetime()))
            .expect("build nodes");
        let transaction = nodes
            .iter()
            .find(|node| node.claim_id == "tra_1")
            .expect("transaction node");
        assert!(transaction.direct_landmark_ids.is_empty());
    }

    #[test]
    fn propagation_does_not_flow_to_normative_or_evaluative() {
        let extraction: GrammaticalExtractionOutput =
            serde_json::from_str(sample_extraction_json()).expect("sample extraction must parse");
        let landmark_id = Uuid::new_v4();
        let tag_to_landmark_id = HashMap::from([(1, landmark_id)]);
        let nodes = build_claim_nodes(&extraction, &tag_to_landmark_id, Some(sample_datetime()))
            .expect("build nodes");
        let effective = compute_effective_landmark_ids(&nodes);

        let effective_by_id = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.claim_id.as_str(), effective[index].clone()))
            .collect::<HashMap<_, _>>();

        assert!(!effective_by_id
            .get("nor_1")
            .expect("nor_1 should exist")
            .contains(&landmark_id));
        assert!(!effective_by_id
            .get("eva_1")
            .expect("eva_1 should exist")
            .contains(&landmark_id));
    }

    #[test]
    fn propagation_flows_from_descriptive_theme_to_units_only() {
        let extraction: GrammaticalExtractionOutput = serde_json::from_str(
            r#"{
                "transactions": [],
                "descriptives": [
                    {
                        "id": "des_1",
                        "object": "unit",
                        "kind": "UNIT",
                        "unit_ids": [],
                        "spans": ["u"],
                        "references_tags_id": []
                    },
                    {
                        "id": "des_2",
                        "object": "theme",
                        "kind": "THEME",
                        "unit_ids": ["des_1"],
                        "spans": ["t"],
                        "references_tags_id": [1]
                    }
                ],
                "normatives": [],
                "evaluatives": []
            }"#,
        )
        .expect("sample extraction must parse");
        let landmark_id = Uuid::new_v4();
        let tag_to_landmark_id = HashMap::from([(1, landmark_id)]);
        let nodes = build_claim_nodes(&extraction, &tag_to_landmark_id, Some(sample_datetime()))
            .expect("build nodes");
        let effective = compute_effective_landmark_ids(&nodes);

        let effective_by_id = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.claim_id.as_str(), effective[index].clone()))
            .collect::<HashMap<_, _>>();

        assert!(effective_by_id
            .get("des_2")
            .expect("des_2 should exist")
            .contains(&landmark_id));
        assert!(effective_by_id
            .get("des_1")
            .expect("des_1 should exist")
            .contains(&landmark_id));
    }

    #[test]
    fn apply_date_offset_handles_relative_and_day_part_offsets() {
        let base = sample_datetime();

        let yesterday = apply_date_offset(Some(base), "YESTERDAY").expect("has date");
        assert_eq!(
            yesterday.date(),
            NaiveDate::from_ymd_opt(2026, 2, 17).expect("valid date")
        );

        let tomorrow = apply_date_offset(Some(base), "TOMORROW").expect("has date");
        assert_eq!(
            tomorrow.date(),
            NaiveDate::from_ymd_opt(2026, 2, 19).expect("valid date")
        );

        let morning = apply_date_offset(Some(base), "TODAY_MORNING").expect("has date");
        assert_eq!(morning.time().hour(), 10);
        assert_eq!(morning.time().minute(), 0);

        let afternoon = apply_date_offset(Some(base), "TODAY_AFTERNOON").expect("has date");
        assert_eq!(afternoon.time().hour(), 15);
        assert_eq!(afternoon.time().minute(), 0);
    }
}
