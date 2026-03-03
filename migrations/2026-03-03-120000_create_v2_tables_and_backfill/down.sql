
DROP TABLE IF EXISTS post_relations;
DROP TABLE IF EXISTS lens_targets;
DROP TABLE IF EXISTS lens_heads;
DROP TABLE IF EXISTS lens_analysis_scopes;
DROP TABLE IF EXISTS landscape_landmarks;
DROP TABLE IF EXISTS landmark_relations;
DROP TABLE IF EXISTS element_landmarks;
DROP TABLE IF EXISTS element_relations;

-- Break cyclic dependency created in up.sql:
-- landscape_analyses.trace_mirror_id -> trace_mirrors.id
ALTER TABLE IF EXISTS landscape_analyses
DROP CONSTRAINT IF EXISTS landscape_analyses_trace_mirror_id_fkey;

DROP TABLE IF EXISTS posts;
DROP TABLE IF EXISTS lenses;
DROP TABLE IF EXISTS "references";
DROP TABLE IF EXISTS elements;
DROP TABLE IF EXISTS trace_mirrors;
DROP TABLE IF EXISTS landmarks;
DROP TABLE IF EXISTS landscape_analyses;
DROP TABLE IF EXISTS traces;
DROP TABLE IF EXISTS journals;

DROP FUNCTION IF EXISTS safe_jsonb_array(TEXT);
DROP FUNCTION IF EXISTS safe_jsonb(TEXT);
DROP FUNCTION IF EXISTS safe_bool(TEXT);
DROP FUNCTION IF EXISTS safe_int(TEXT);
DROP FUNCTION IF EXISTS safe_uuid(TEXT);
