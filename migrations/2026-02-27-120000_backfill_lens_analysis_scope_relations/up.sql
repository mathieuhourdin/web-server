WITH RECURSIVE lens_heads AS (
    SELECT rr.origin_resource_id AS lens_id,
           rr.target_resource_id AS analysis_id,
           rr.user_id AS user_id
    FROM resource_relations rr
    INNER JOIN resources lens_r ON lens_r.id = rr.origin_resource_id
    INNER JOIN resources analysis_r ON analysis_r.id = rr.target_resource_id
    WHERE rr.relation_type = 'head'
      AND lens_r.entity_type = 'lens'
      AND analysis_r.entity_type = 'lnds'
),
analysis_lineage AS (
    SELECT lh.lens_id,
           lh.analysis_id,
           lh.user_id
    FROM lens_heads lh

    UNION

    SELECT al.lens_id,
           parent_rel.target_resource_id AS analysis_id,
           al.user_id
    FROM analysis_lineage al
    INNER JOIN resource_relations parent_rel
        ON parent_rel.origin_resource_id = al.analysis_id
       AND parent_rel.relation_type = 'prnt'
    INNER JOIN resources parent_analysis_r
        ON parent_analysis_r.id = parent_rel.target_resource_id
    WHERE parent_analysis_r.entity_type = 'lnds'
),
unique_scope AS (
    SELECT DISTINCT lens_id, analysis_id, user_id
    FROM analysis_lineage
)
INSERT INTO resource_relations (
    origin_resource_id,
    target_resource_id,
    relation_comment,
    created_at,
    updated_at,
    user_id,
    relation_type
)
SELECT us.lens_id,
       us.analysis_id,
       '',
       now(),
       now(),
       us.user_id,
       'lnsa'
FROM unique_scope us
WHERE NOT EXISTS (
    SELECT 1
    FROM resource_relations existing
    WHERE existing.origin_resource_id = us.lens_id
      AND existing.target_resource_id = us.analysis_id
      AND existing.relation_type = 'lnsa'
);
