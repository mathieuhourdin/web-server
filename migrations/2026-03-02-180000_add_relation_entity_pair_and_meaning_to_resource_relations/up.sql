ALTER TABLE resource_relations
ADD COLUMN relation_entity_pair TEXT,
ADD COLUMN relation_meaning TEXT;

UPDATE resource_relations rr
SET relation_entity_pair = CASE
    WHEN origin_resource.entity_type = 'trcm' AND target_resource.entity_type = 'lnds' THEN 'TRACE_MIRROR_TO_LANDSCAPE_ANALYSIS'
    WHEN origin_resource.entity_type = 'trcm' AND target_resource.entity_type = 'trce' THEN 'TRACE_MIRROR_TO_TRACE'
    WHEN origin_resource.entity_type = 'lnds' AND target_resource.entity_type = 'trce' THEN 'LANDSCAPE_ANALYSIS_TO_TRACE'
    WHEN origin_resource.entity_type = 'lnds' AND target_resource.entity_type = 'lnds' THEN 'LANDSCAPE_ANALYSIS_TO_LANDSCAPE_ANALYSIS'
    WHEN origin_resource.entity_type = 'elmt' AND target_resource.entity_type = 'lnds' THEN 'ELEMENT_TO_LANDSCAPE_ANALYSIS'
    WHEN origin_resource.entity_type = 'elmt' AND target_resource.entity_type = 'trce' THEN 'ELEMENT_TO_TRACE'
    WHEN origin_resource.entity_type = 'elmt' AND target_resource.entity_type = 'trcm' THEN 'ELEMENT_TO_TRACE_MIRROR'
    WHEN origin_resource.entity_type = 'elmt' AND target_resource.entity_type = 'lndm' THEN 'ELEMENT_TO_LANDMARK'
    WHEN origin_resource.entity_type = 'elmt' AND target_resource.entity_type = 'elmt' THEN 'ELEMENT_TO_ELEMENT'
    WHEN origin_resource.entity_type = 'lndm' AND target_resource.entity_type = 'lnds' THEN 'LANDMARK_TO_LANDSCAPE_ANALYSIS'
    WHEN origin_resource.entity_type = 'lndm' AND target_resource.entity_type = 'lndm' THEN 'LANDMARK_TO_LANDMARK'
    WHEN origin_resource.entity_type = 'trce' AND target_resource.entity_type = 'jrnl' THEN 'TRACE_TO_JOURNAL'
    WHEN origin_resource.entity_type = 'lens' AND target_resource.entity_type = 'lnds' THEN 'LENS_TO_LANDSCAPE_ANALYSIS'
    WHEN origin_resource.entity_type = 'lens' AND target_resource.entity_type = 'trce' THEN 'LENS_TO_TRACE'
    WHEN origin_resource.entity_type = 'trcm' AND target_resource.entity_type = 'lndm' THEN 'TRACE_MIRROR_TO_LANDMARK'
    WHEN origin_resource.entity_type = 'ppst' AND target_resource.entity_type = 'ppst' THEN 'PUBLIC_POST_TO_PUBLIC_POST'
    ELSE 'UNKNOWN'
END,
relation_meaning = CASE
    WHEN rr.relation_type = 'lnds' AND origin_resource.entity_type = 'trcm' AND target_resource.entity_type = 'lnds' THEN 'ATTACHED_TO_LANDSCAPE'
    WHEN rr.relation_type = 'refr' AND origin_resource.entity_type = 'lndm' AND target_resource.entity_type = 'lnds' THEN 'REFERENCED'
    WHEN rr.relation_type = 'trce' AND origin_resource.entity_type = 'trcm' AND target_resource.entity_type = 'trce' THEN 'MIRRORS'
    WHEN rr.relation_type = 'trce' AND origin_resource.entity_type = 'lnds' AND target_resource.entity_type = 'trce' THEN 'ANALYZES'
    WHEN rr.relation_type = 'elmt' AND origin_resource.entity_type = 'elmt' AND target_resource.entity_type = 'trce' THEN 'EXTRACTED_FROM'
    WHEN rr.relation_type = 'trcm' AND origin_resource.entity_type = 'elmt' AND target_resource.entity_type = 'trcm' THEN 'EXTRACTED_IN'
    WHEN rr.relation_type = 'ownr' AND origin_resource.entity_type = 'lndm' AND target_resource.entity_type = 'lnds' THEN 'OWNED_BY_ANALYSIS'
    WHEN rr.relation_type = 'ownr' AND origin_resource.entity_type = 'elmt' AND target_resource.entity_type = 'lnds' THEN 'OWNED_BY_ANALYSIS'
    WHEN rr.relation_type = 'lnsa' AND origin_resource.entity_type = 'lens' AND target_resource.entity_type = 'lnds' THEN 'INCLUDES_IN_SCOPE'
    WHEN rr.relation_type = 'jrit' AND origin_resource.entity_type = 'trce' AND target_resource.entity_type = 'jrnl' THEN 'JOURNAL_ITEM_OF'
    WHEN rr.relation_type = 'theme_of' AND origin_resource.entity_type = 'elmt' AND target_resource.entity_type = 'elmt' THEN 'THEME_OF'
    WHEN rr.relation_type = 'applies_to' AND origin_resource.entity_type = 'elmt' AND target_resource.entity_type = 'elmt' THEN 'APPLIES_TO'
    WHEN rr.relation_type = 'head' AND origin_resource.entity_type = 'lens' AND target_resource.entity_type = 'lnds' THEN 'HAS_CURRENT_HEAD'
    WHEN rr.relation_type = 'hlpr' AND origin_resource.entity_type = 'lndm' AND target_resource.entity_type = 'lndm' THEN 'HIGH_LEVEL_PROJECT_RELATED_TO'
    WHEN rr.relation_type = 'elmt' AND origin_resource.entity_type = 'elmt' AND target_resource.entity_type = 'lndm' THEN 'INVOLVES'
    WHEN rr.relation_type = 'subtask_of' AND origin_resource.entity_type = 'elmt' AND target_resource.entity_type = 'elmt' THEN 'SUBTASK_OF'
    WHEN rr.relation_type = 'rfrr' AND origin_resource.entity_type = 'trcm' AND target_resource.entity_type = 'lndm' THEN 'REFERENCE_MENTION'
    WHEN rr.relation_type = 'trgt' AND origin_resource.entity_type = 'lens' AND target_resource.entity_type = 'trce' THEN 'TARGETS_TRACE'
    WHEN rr.relation_type = 'bibl' AND origin_resource.entity_type = 'ppst' AND target_resource.entity_type = 'ppst' THEN 'BIBLIOGRAPHY'
    WHEN rr.relation_type = 'rply' AND origin_resource.entity_type = 'lnds' AND target_resource.entity_type = 'lnds' THEN 'REPLAYED_FROM'
    WHEN rr.relation_type = 'prir' AND origin_resource.entity_type = 'trcm' AND target_resource.entity_type = 'lndm' THEN 'HAS_PRIMARY_LANDMARK'
    WHEN rr.relation_type = 'prit' AND origin_resource.entity_type = 'trcm' AND target_resource.entity_type = 'lndm' THEN 'HAS_PRIMARY_THEME'
    WHEN rr.relation_type = 'fork' AND origin_resource.entity_type = 'lens' AND target_resource.entity_type = 'lnds' THEN 'FORKED_FROM'
    WHEN rr.relation_type = 'prnt' AND origin_resource.entity_type = 'lnds' AND target_resource.entity_type = 'lnds' THEN 'CHILD_OF'
    WHEN rr.relation_type = 'prnt' AND origin_resource.entity_type = 'lndm' AND target_resource.entity_type = 'lndm' THEN 'CHILD_OF'
    ELSE 'UNKNOWN'
END
FROM resources origin_resource,
     resources target_resource
WHERE rr.origin_resource_id = origin_resource.id
  AND rr.target_resource_id = target_resource.id;

UPDATE resource_relations
SET relation_entity_pair = 'UNKNOWN'
WHERE relation_entity_pair IS NULL;

UPDATE resource_relations
SET relation_meaning = 'UNKNOWN'
WHERE relation_meaning IS NULL;

ALTER TABLE resource_relations
ALTER COLUMN relation_entity_pair SET DEFAULT 'UNKNOWN',
ALTER COLUMN relation_meaning SET DEFAULT 'UNKNOWN',
ALTER COLUMN relation_entity_pair SET NOT NULL,
ALTER COLUMN relation_meaning SET NOT NULL;
