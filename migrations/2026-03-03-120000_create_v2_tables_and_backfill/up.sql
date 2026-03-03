BEGIN;

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ---------------------------------------------------------------------------
-- Helper coercion functions (skip/coerce strategy)
-- ---------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION safe_uuid(input_text TEXT)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    IF input_text IS NULL OR btrim(input_text) = '' THEN
        RETURN NULL;
    END IF;
    RETURN input_text::UUID;
EXCEPTION WHEN OTHERS THEN
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION safe_int(input_text TEXT)
RETURNS INT
LANGUAGE plpgsql
AS $$
BEGIN
    IF input_text IS NULL OR btrim(input_text) = '' THEN
        RETURN NULL;
    END IF;
    RETURN input_text::INT;
EXCEPTION WHEN OTHERS THEN
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION safe_bool(input_text TEXT)
RETURNS BOOLEAN
LANGUAGE plpgsql
AS $$
DECLARE
    normalized TEXT;
BEGIN
    IF input_text IS NULL OR btrim(input_text) = '' THEN
        RETURN NULL;
    END IF;

    normalized := lower(btrim(input_text));

    IF normalized IN ('true', 't', '1', 'yes', 'y') THEN
        RETURN TRUE;
    ELSIF normalized IN ('false', 'f', '0', 'no', 'n') THEN
        RETURN FALSE;
    END IF;

    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION safe_jsonb(input_text TEXT)
RETURNS JSONB
LANGUAGE plpgsql
AS $$
BEGIN
    IF input_text IS NULL OR btrim(input_text) = '' THEN
        RETURN '{}'::JSONB;
    END IF;
    RETURN input_text::JSONB;
EXCEPTION WHEN OTHERS THEN
    RETURN '{}'::JSONB;
END;
$$;

CREATE OR REPLACE FUNCTION safe_jsonb_array(input_text TEXT)
RETURNS JSONB
LANGUAGE plpgsql
AS $$
DECLARE
    parsed JSONB;
BEGIN
    parsed := safe_jsonb(input_text);
    IF jsonb_typeof(parsed) = 'array' THEN
        RETURN parsed;
    END IF;
    RETURN '[]'::JSONB;
END;
$$;

-- ---------------------------------------------------------------------------
-- V2 entity tables
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS journals (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    subtitle TEXT NOT NULL,
    content TEXT NOT NULL,
    journal_type TEXT NOT NULL CHECK (journal_type IN ('META_JOURNAL', 'WORK_LOG_JOURNAL', 'READING_NOTE_JOURNAL')),
    status TEXT NOT NULL CHECK (status IN ('DRAFT', 'PUBLISHED', 'ARCHIVED')),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS traces (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    journal_id UUID NOT NULL REFERENCES journals(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    subtitle TEXT NOT NULL,
    content TEXT NOT NULL,
    interaction_date TIMESTAMP NULL,
    trace_type TEXT NOT NULL CHECK (trace_type IN ('USER_TRACE', 'BIO_TRACE', 'WORKSPACE_TRACE', 'HIGH_LEVEL_PROJECTS_DEFINITION')),
    status TEXT NOT NULL CHECK (status IN ('DRAFT', 'FINALIZED', 'ARCHIVED')),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS landscape_analyses (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    subtitle TEXT NOT NULL,
    plain_text_state_summary TEXT NOT NULL,
    interaction_date TIMESTAMP NULL,
    processing_state TEXT NOT NULL CHECK (processing_state IN ('PENDING', 'REPLAY_REQUESTED', 'COMPLETED')),
    parent_id UUID NULL REFERENCES landscape_analyses(id) ON DELETE SET NULL,
    replayed_from_id UUID NULL REFERENCES landscape_analyses(id) ON DELETE SET NULL,
    analyzed_trace_id UUID NULL REFERENCES traces(id) ON DELETE SET NULL,
    trace_mirror_id UUID NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS landmarks (
    id UUID PRIMARY KEY,
    analysis_id UUID NOT NULL REFERENCES landscape_analyses(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    parent_id UUID NULL REFERENCES landmarks(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    subtitle TEXT NOT NULL,
    content TEXT NOT NULL,
    external_content_url TEXT NULL,
    comment TEXT NULL,
    image_url TEXT NULL,
    landmark_type TEXT NOT NULL,
    maturing_state TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS trace_mirrors (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    trace_id UUID NOT NULL REFERENCES traces(id) ON DELETE CASCADE,
    landscape_analysis_id UUID NOT NULL REFERENCES landscape_analyses(id) ON DELETE CASCADE,
    primary_landmark_id UUID NULL REFERENCES landmarks(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    subtitle TEXT NOT NULL,
    content TEXT NOT NULL,
    trace_mirror_type TEXT NOT NULL CHECK (trace_mirror_type IN ('NOTE', 'JOURNAL', 'HIGH_LEVEL_PROJECTS', 'BIO')),
    tags JSONB NOT NULL DEFAULT '[]'::JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

ALTER TABLE landscape_analyses
DROP CONSTRAINT IF EXISTS landscape_analyses_trace_mirror_id_fkey;

ALTER TABLE landscape_analyses
ADD CONSTRAINT landscape_analyses_trace_mirror_id_fkey
FOREIGN KEY (trace_mirror_id)
REFERENCES trace_mirrors(id)
ON DELETE SET NULL;

CREATE TABLE IF NOT EXISTS elements (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    analysis_id UUID NOT NULL REFERENCES landscape_analyses(id) ON DELETE CASCADE,
    trace_id UUID NOT NULL REFERENCES traces(id) ON DELETE CASCADE,
    trace_mirror_id UUID NULL REFERENCES trace_mirrors(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    subtitle TEXT NOT NULL,
    content TEXT NOT NULL,
    extended_content TEXT NULL,
    verb TEXT NOT NULL,
    element_type TEXT NOT NULL CHECK (element_type IN ('TRANSACTION', 'DESCRIPTIVE', 'NORMATIVE', 'EVALUATIVE')),
    element_subtype TEXT NOT NULL,
    interaction_date TIMESTAMP NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "references" (
    id UUID PRIMARY KEY,
    tag_id INT NOT NULL,
    trace_mirror_id UUID NOT NULL REFERENCES trace_mirrors(id) ON DELETE CASCADE,
    landmark_id UUID NULL REFERENCES landmarks(id) ON DELETE SET NULL,
    landscape_analysis_id UUID NOT NULL REFERENCES landscape_analyses(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    mention TEXT NOT NULL,
    reference_type TEXT NOT NULL,
    context_tags JSONB NOT NULL DEFAULT '[]'::JSONB,
    reference_variants JSONB NOT NULL DEFAULT '[]'::JSONB,
    parent_reference_id UUID NULL REFERENCES "references"(id) ON DELETE SET NULL,
    is_user_specific BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS lenses (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    processing_state TEXT NOT NULL CHECK (processing_state IN ('OUT_OF_SYNC', 'IN_SYNC')),
    fork_landscape_id UUID NULL REFERENCES landscape_analyses(id) ON DELETE SET NULL,
    current_landscape_id UUID NULL REFERENCES landscape_analyses(id) ON DELETE SET NULL,
    target_trace_id UUID NULL REFERENCES traces(id) ON DELETE SET NULL,
    autoplay BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS posts (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    subtitle TEXT NOT NULL,
    content TEXT NOT NULL,
    image_url TEXT NULL,
    post_type TEXT NOT NULL CHECK (post_type IN ('OUTPUT', 'INPUT', 'PROBLEM', 'WISH')),
    publishing_date TIMESTAMP NULL,
    publishing_state TEXT NOT NULL,
    maturing_state TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- ---------------------------------------------------------------------------
-- V2 relation tables
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS element_relations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    origin_element_id UUID NOT NULL REFERENCES elements(id) ON DELETE CASCADE,
    target_element_id UUID NOT NULL REFERENCES elements(id) ON DELETE CASCADE,
    relation_type TEXT NOT NULL CHECK (relation_type IN ('APPLIES_TO', 'SUBTASK_OF', 'THEME_OF')),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE (origin_element_id, target_element_id, relation_type)
);

CREATE TABLE IF NOT EXISTS element_landmarks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    element_id UUID NOT NULL REFERENCES elements(id) ON DELETE CASCADE,
    landmark_id UUID NOT NULL REFERENCES landmarks(id) ON DELETE CASCADE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE (element_id, landmark_id)
);

CREATE TABLE IF NOT EXISTS landmark_relations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    origin_landmark_id UUID NOT NULL REFERENCES landmarks(id) ON DELETE CASCADE,
    target_landmark_id UUID NOT NULL REFERENCES landmarks(id) ON DELETE CASCADE,
    relation_type TEXT NOT NULL CHECK (relation_type IN ('CHILD_OF', 'HIGH_LEVEL_PROJECT_RELATED_TO')),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE (origin_landmark_id, target_landmark_id, relation_type)
);

CREATE TABLE IF NOT EXISTS landscape_landmarks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    landscape_analysis_id UUID NOT NULL REFERENCES landscape_analyses(id) ON DELETE CASCADE,
    landmark_id UUID NOT NULL REFERENCES landmarks(id) ON DELETE CASCADE,
    relation_type TEXT NOT NULL CHECK (relation_type IN ('OWNED_BY_ANALYSIS', 'REFERENCED')),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE (landscape_analysis_id, landmark_id, relation_type)
);

CREATE TABLE IF NOT EXISTS lens_analysis_scopes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    lens_id UUID NOT NULL REFERENCES lenses(id) ON DELETE CASCADE,
    landscape_analysis_id UUID NOT NULL REFERENCES landscape_analyses(id) ON DELETE CASCADE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE (lens_id, landscape_analysis_id)
);

CREATE TABLE IF NOT EXISTS lens_heads (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    lens_id UUID NOT NULL REFERENCES lenses(id) ON DELETE CASCADE,
    landscape_analysis_id UUID NOT NULL REFERENCES landscape_analyses(id) ON DELETE CASCADE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE (lens_id)
);

CREATE TABLE IF NOT EXISTS lens_targets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    lens_id UUID NOT NULL REFERENCES lenses(id) ON DELETE CASCADE,
    trace_id UUID NOT NULL REFERENCES traces(id) ON DELETE CASCADE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE (lens_id)
);

CREATE TABLE IF NOT EXISTS post_relations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    origin_post_id UUID NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    target_post_id UUID NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    relation_type TEXT NOT NULL CHECK (relation_type IN ('BIBLIOGRAPHY')),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE (origin_post_id, target_post_id, relation_type)
);

-- ---------------------------------------------------------------------------
-- Indexes
-- ---------------------------------------------------------------------------

CREATE INDEX IF NOT EXISTS idx_journals_user_updated_at ON journals(user_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_traces_user_interaction_date ON traces(user_id, interaction_date DESC);
CREATE INDEX IF NOT EXISTS idx_traces_journal_interaction_date ON traces(journal_id, interaction_date DESC);

CREATE INDEX IF NOT EXISTS idx_landscape_analyses_user_date ON landscape_analyses(user_id, interaction_date DESC);
CREATE INDEX IF NOT EXISTS idx_trace_mirrors_landscape ON trace_mirrors(landscape_analysis_id);
CREATE INDEX IF NOT EXISTS idx_trace_mirrors_trace ON trace_mirrors(trace_id);

CREATE INDEX IF NOT EXISTS idx_elements_analysis ON elements(analysis_id);
CREATE INDEX IF NOT EXISTS idx_elements_trace ON elements(trace_id);
CREATE INDEX IF NOT EXISTS idx_elements_trace_mirror ON elements(trace_mirror_id);

CREATE INDEX IF NOT EXISTS idx_references_trace_mirror ON "references"(trace_mirror_id);
CREATE INDEX IF NOT EXISTS idx_references_landmark ON "references"(landmark_id);
CREATE INDEX IF NOT EXISTS idx_references_landscape ON "references"(landscape_analysis_id);

CREATE INDEX IF NOT EXISTS idx_landmarks_analysis ON landmarks(analysis_id);
CREATE INDEX IF NOT EXISTS idx_landmarks_parent ON landmarks(parent_id);

CREATE INDEX IF NOT EXISTS idx_lenses_user ON lenses(user_id);
CREATE INDEX IF NOT EXISTS idx_posts_user_date ON posts(user_id, publishing_date DESC);

-- ---------------------------------------------------------------------------
-- Automatic updated_at triggers
-- ---------------------------------------------------------------------------

SELECT diesel_manage_updated_at('journals');
SELECT diesel_manage_updated_at('traces');
SELECT diesel_manage_updated_at('landscape_analyses');
SELECT diesel_manage_updated_at('landmarks');
SELECT diesel_manage_updated_at('trace_mirrors');
SELECT diesel_manage_updated_at('elements');
SELECT diesel_manage_updated_at('"references"');
SELECT diesel_manage_updated_at('lenses');
SELECT diesel_manage_updated_at('posts');
SELECT diesel_manage_updated_at('element_relations');
SELECT diesel_manage_updated_at('element_landmarks');
SELECT diesel_manage_updated_at('landmark_relations');
SELECT diesel_manage_updated_at('landscape_landmarks');
SELECT diesel_manage_updated_at('lens_analysis_scopes');
SELECT diesel_manage_updated_at('lens_heads');
SELECT diesel_manage_updated_at('lens_targets');
SELECT diesel_manage_updated_at('post_relations');

-- ---------------------------------------------------------------------------
-- Backfill entity tables (skip/coerce)
-- ---------------------------------------------------------------------------

INSERT INTO journals (
    id,
    user_id,
    title,
    subtitle,
    content,
    journal_type,
    status,
    created_at,
    updated_at
)
SELECT
    r.id,
    outp.interaction_user_id,
    COALESCE(r.title, ''),
    COALESCE(r.subtitle, ''),
    COALESCE(r.content, ''),
    CASE
        WHEN r.resource_type = 'meta' THEN 'META_JOURNAL'
        WHEN r.resource_type = 'rjnl' THEN 'READING_NOTE_JOURNAL'
        WHEN r.resource_type = 'wjnl' THEN 'WORK_LOG_JOURNAL'
        ELSE 'WORK_LOG_JOURNAL'
    END,
    CASE
        WHEN r.maturing_state = 'trsh' THEN 'ARCHIVED'
        WHEN r.publishing_state = 'pbsh' OR r.maturing_state = 'fnsh' THEN 'PUBLISHED'
        ELSE 'DRAFT'
    END,
    r.created_at,
    r.updated_at
FROM resources r
LEFT JOIN LATERAL (
    SELECT i.interaction_user_id
    FROM interactions i
    WHERE i.resource_id = r.id
      AND i.interaction_type = 'outp'
    ORDER BY i.interaction_date DESC, i.created_at DESC
    LIMIT 1
) outp ON TRUE
WHERE r.entity_type = 'jrnl'
  AND outp.interaction_user_id IS NOT NULL
ON CONFLICT (id) DO NOTHING;

INSERT INTO traces (
    id,
    user_id,
    journal_id,
    title,
    subtitle,
    content,
    interaction_date,
    trace_type,
    status,
    created_at,
    updated_at
)
SELECT
    r.id,
    outp.interaction_user_id,
    jrel.journal_id,
    COALESCE(r.title, ''),
    COALESCE(r.subtitle, ''),
    COALESCE(r.content, ''),
    COALESCE(outp.interaction_date, r.created_at),
    CASE
        WHEN r.resource_type = 'btrc' THEN 'BIO_TRACE'
        WHEN r.resource_type = 'wtrc' THEN 'WORKSPACE_TRACE'
        WHEN r.resource_type = 'hlpd' THEN 'HIGH_LEVEL_PROJECTS_DEFINITION'
        ELSE 'USER_TRACE'
    END,
    CASE
        WHEN r.maturing_state IN ('drft', 'rvew', 'rply') THEN 'DRAFT'
        WHEN r.maturing_state = 'trsh' THEN 'ARCHIVED'
        ELSE 'FINALIZED'
    END,
    r.created_at,
    r.updated_at
FROM resources r
LEFT JOIN LATERAL (
    SELECT i.interaction_user_id, i.interaction_date
    FROM interactions i
    WHERE i.resource_id = r.id
      AND i.interaction_type = 'outp'
    ORDER BY i.interaction_date DESC, i.created_at DESC
    LIMIT 1
) outp ON TRUE
JOIN LATERAL (
    SELECT rr.target_resource_id AS journal_id
    FROM resource_relations rr
    WHERE rr.origin_resource_id = r.id
      AND rr.relation_type = 'jrit'
    ORDER BY rr.updated_at DESC, rr.created_at DESC
    LIMIT 1
) jrel ON TRUE
JOIN journals j ON j.id = jrel.journal_id
WHERE r.entity_type = 'trce'
  AND outp.interaction_user_id IS NOT NULL
ON CONFLICT (id) DO NOTHING;

INSERT INTO landscape_analyses (
    id,
    user_id,
    title,
    subtitle,
    plain_text_state_summary,
    interaction_date,
    processing_state,
    parent_id,
    replayed_from_id,
    analyzed_trace_id,
    trace_mirror_id,
    created_at,
    updated_at
)
SELECT
    r.id,
    COALESCE(outp.interaction_user_id, parent_rel.user_id, replay_rel.user_id, analyzed_rel.user_id),
    COALESCE(r.title, ''),
    COALESCE(r.subtitle, ''),
    COALESCE(r.content, ''),
    COALESCE(outp.interaction_date, r.created_at),
    CASE
        WHEN r.maturing_state = 'rply' THEN 'REPLAY_REQUESTED'
        WHEN r.maturing_state = 'fnsh' THEN 'COMPLETED'
        ELSE 'PENDING'
    END,
    parent_rel.parent_id,
    replay_rel.replayed_from_id,
    analyzed_rel.analyzed_trace_id,
    NULL,
    r.created_at,
    r.updated_at
FROM resources r
LEFT JOIN LATERAL (
    SELECT i.interaction_user_id, i.interaction_date
    FROM interactions i
    WHERE i.resource_id = r.id
      AND i.interaction_type = 'outp'
    ORDER BY i.interaction_date DESC, i.created_at DESC
    LIMIT 1
) outp ON TRUE
LEFT JOIN LATERAL (
    SELECT rr.target_resource_id AS parent_id, rr.user_id
    FROM resource_relations rr
    WHERE rr.origin_resource_id = r.id
      AND rr.relation_type = 'prnt'
      AND rr.target_resource_id IN (SELECT id FROM resources WHERE entity_type = 'lnds')
    ORDER BY rr.updated_at DESC, rr.created_at DESC
    LIMIT 1
) parent_rel ON TRUE
LEFT JOIN LATERAL (
    SELECT rr.target_resource_id AS replayed_from_id, rr.user_id
    FROM resource_relations rr
    WHERE rr.origin_resource_id = r.id
      AND rr.relation_type = 'rply'
      AND rr.target_resource_id IN (SELECT id FROM resources WHERE entity_type = 'lnds')
    ORDER BY rr.updated_at DESC, rr.created_at DESC
    LIMIT 1
) replay_rel ON TRUE
LEFT JOIN LATERAL (
    SELECT rr.target_resource_id AS analyzed_trace_id, rr.user_id
    FROM resource_relations rr
    JOIN traces t ON t.id = rr.target_resource_id
    WHERE rr.origin_resource_id = r.id
      AND rr.relation_type = 'trce'
    ORDER BY rr.updated_at DESC, rr.created_at DESC
    LIMIT 1
) analyzed_rel ON TRUE
WHERE r.entity_type = 'lnds'
  AND COALESCE(outp.interaction_user_id, parent_rel.user_id, replay_rel.user_id, analyzed_rel.user_id) IS NOT NULL
ON CONFLICT (id) DO NOTHING;

INSERT INTO landmarks (
    id,
    analysis_id,
    user_id,
    parent_id,
    title,
    subtitle,
    content,
    external_content_url,
    comment,
    image_url,
    landmark_type,
    maturing_state,
    created_at,
    updated_at
)
SELECT
    r.id,
    COALESCE(ownr_rel.analysis_id, refr_rel.analysis_id),
    COALESCE(outp.interaction_user_id, ownr_rel.user_id, refr_rel.user_id),
    parent_rel.parent_id,
    COALESCE(r.title, ''),
    COALESCE(r.subtitle, ''),
    COALESCE(r.content, ''),
    r.external_content_url,
    r.comment,
    r.image_url,
    CASE
        WHEN r.resource_type = 'miss' THEN 'PROJECT'
        WHEN r.resource_type = 'hlpr' THEN 'HIGH_LEVEL_PROJECT'
        WHEN r.resource_type = 'dlvr' THEN 'DELIVERABLE'
        WHEN r.resource_type = 'autr' THEN 'PERSON'
        WHEN r.resource_type = 'them' THEN 'TOPIC'
        WHEN r.resource_type = 'qest' THEN 'QUESTION'
        ELSE 'RESOURCE'
    END,
    COALESCE(r.maturing_state, 'drft'),
    r.created_at,
    r.updated_at
FROM resources r
LEFT JOIN LATERAL (
    SELECT i.interaction_user_id
    FROM interactions i
    WHERE i.resource_id = r.id
    ORDER BY i.interaction_date DESC, i.created_at DESC
    LIMIT 1
) outp ON TRUE
LEFT JOIN LATERAL (
    SELECT rr.target_resource_id AS analysis_id, rr.user_id
    FROM resource_relations rr
    JOIN landscape_analyses l ON l.id = rr.target_resource_id
    WHERE rr.origin_resource_id = r.id
      AND rr.relation_type = 'ownr'
    ORDER BY rr.updated_at DESC, rr.created_at DESC
    LIMIT 1
) ownr_rel ON TRUE
LEFT JOIN LATERAL (
    SELECT rr.target_resource_id AS analysis_id, rr.user_id
    FROM resource_relations rr
    JOIN landscape_analyses l ON l.id = rr.target_resource_id
    WHERE rr.origin_resource_id = r.id
      AND rr.relation_type = 'refr'
    ORDER BY rr.updated_at DESC, rr.created_at DESC
    LIMIT 1
) refr_rel ON TRUE
LEFT JOIN LATERAL (
    SELECT rr.target_resource_id AS parent_id
    FROM resource_relations rr
    WHERE rr.origin_resource_id = r.id
      AND rr.relation_type = 'prnt'
      AND rr.target_resource_id IN (SELECT id FROM resources WHERE entity_type = 'lndm')
    ORDER BY rr.updated_at DESC, rr.created_at DESC
    LIMIT 1
) parent_rel ON TRUE
WHERE r.entity_type = 'lndm'
  AND COALESCE(ownr_rel.analysis_id, refr_rel.analysis_id) IS NOT NULL
  AND COALESCE(outp.interaction_user_id, ownr_rel.user_id, refr_rel.user_id) IS NOT NULL
ON CONFLICT (id) DO NOTHING;

INSERT INTO trace_mirrors (
    id,
    user_id,
    trace_id,
    landscape_analysis_id,
    primary_landmark_id,
    title,
    subtitle,
    content,
    trace_mirror_type,
    tags,
    created_at,
    updated_at
)
SELECT
    r.id,
    COALESCE(outp.interaction_user_id, lrel.user_id, trel.user_id),
    trel.trace_id,
    lrel.landscape_id,
    prir_rel.primary_landmark_id,
    COALESCE(r.title, ''),
    COALESCE(r.subtitle, ''),
    COALESCE(r.content, ''),
    CASE
        WHEN r.resource_type = 'trmj' THEN 'JOURNAL'
        WHEN r.resource_type = 'trmb' THEN 'BIO'
        WHEN r.resource_type = 'trmh' THEN 'HIGH_LEVEL_PROJECTS'
        ELSE 'NOTE'
    END,
    safe_jsonb_array(r.comment),
    r.created_at,
    r.updated_at
FROM resources r
LEFT JOIN LATERAL (
    SELECT i.interaction_user_id
    FROM interactions i
    WHERE i.resource_id = r.id
      AND i.interaction_type = 'outp'
    ORDER BY i.interaction_date DESC, i.created_at DESC
    LIMIT 1
) outp ON TRUE
JOIN LATERAL (
    SELECT rr.target_resource_id AS trace_id, rr.user_id
    FROM resource_relations rr
    JOIN traces t ON t.id = rr.target_resource_id
    WHERE rr.origin_resource_id = r.id
      AND rr.relation_type = 'trce'
    ORDER BY rr.updated_at DESC, rr.created_at DESC
    LIMIT 1
) trel ON TRUE
JOIN LATERAL (
    SELECT rr.target_resource_id AS landscape_id, rr.user_id
    FROM resource_relations rr
    JOIN landscape_analyses l ON l.id = rr.target_resource_id
    WHERE rr.origin_resource_id = r.id
      AND rr.relation_type = 'lnds'
    ORDER BY rr.updated_at DESC, rr.created_at DESC
    LIMIT 1
) lrel ON TRUE
LEFT JOIN LATERAL (
    SELECT rr.target_resource_id AS primary_landmark_id
    FROM resource_relations rr
    JOIN landmarks lm ON lm.id = rr.target_resource_id
    WHERE rr.origin_resource_id = r.id
      AND rr.relation_type = 'prir'
    ORDER BY rr.updated_at DESC, rr.created_at DESC
    LIMIT 1
) prir_rel ON TRUE
WHERE r.entity_type = 'trcm'
  AND COALESCE(outp.interaction_user_id, lrel.user_id, trel.user_id) IS NOT NULL
ON CONFLICT (id) DO NOTHING;

UPDATE landscape_analyses la
SET trace_mirror_id = rel.trace_mirror_id
FROM (
    SELECT DISTINCT ON (rr.target_resource_id)
        rr.target_resource_id AS landscape_id,
        rr.origin_resource_id AS trace_mirror_id
    FROM resource_relations rr
    JOIN trace_mirrors tm ON tm.id = rr.origin_resource_id
    WHERE rr.relation_type = 'lnds'
    ORDER BY rr.target_resource_id, rr.updated_at DESC, rr.created_at DESC
) rel
WHERE la.id = rel.landscape_id;

INSERT INTO elements (
    id,
    user_id,
    analysis_id,
    trace_id,
    trace_mirror_id,
    title,
    subtitle,
    content,
    extended_content,
    verb,
    element_type,
    element_subtype,
    interaction_date,
    created_at,
    updated_at
)
SELECT
    r.id,
    COALESCE(outp.interaction_user_id, ownr_rel.user_id),
    ownr_rel.analysis_id,
    trace_rel.trace_id,
    trcm_rel.trace_mirror_id,
    COALESCE(r.title, ''),
    COALESCE(r.subtitle, ''),
    COALESCE(r.content, ''),
    NULL,
    COALESCE(r.comment, ''),
    CASE
        WHEN r.resource_type = 'evnt' THEN 'TRANSACTION'
        WHEN r.resource_type = 'cmnt' THEN 'DESCRIPTIVE'
        WHEN r.resource_type = 'feln' THEN 'EVALUATIVE'
        ELSE 'NORMATIVE'
    END,
    CASE
        WHEN lower(COALESCE(r.resource_subtype, '')) = 'input' THEN 'INPUT'
        WHEN lower(COALESCE(r.resource_subtype, '')) = 'output' THEN 'OUTPUT'
        WHEN lower(COALESCE(r.resource_subtype, '')) = 'transformation' THEN 'TRANSFORMATION'
        WHEN lower(COALESCE(r.resource_subtype, '')) = 'transaction_question' THEN 'TRANSACTION_QUESTION'
        WHEN lower(COALESCE(r.resource_subtype, '')) = 'unit' THEN 'UNIT'
        WHEN lower(COALESCE(r.resource_subtype, '')) = 'descriptive_question' THEN 'DESCRIPTIVE_QUESTION'
        WHEN lower(COALESCE(r.resource_subtype, '')) = 'theme' THEN 'THEME'
        WHEN lower(COALESCE(r.resource_subtype, '')) = 'plan' THEN 'PLAN'
        WHEN lower(COALESCE(r.resource_subtype, '')) = 'obligation' THEN 'OBLIGATION'
        WHEN lower(COALESCE(r.resource_subtype, '')) = 'recommendation' THEN 'RECOMMENDATION'
        WHEN lower(COALESCE(r.resource_subtype, '')) = 'principle' THEN 'PRINCIPLE'
        WHEN lower(COALESCE(r.resource_subtype, '')) = 'emotion' THEN 'EMOTION'
        WHEN lower(COALESCE(r.resource_subtype, '')) = 'energy' THEN 'ENERGY'
        WHEN lower(COALESCE(r.resource_subtype, '')) = 'quality' THEN 'QUALITY'
        WHEN lower(COALESCE(r.resource_subtype, '')) = 'interest' THEN 'INTEREST'
        WHEN r.resource_type = 'evnt' THEN 'OUTPUT'
        WHEN r.resource_type = 'cmnt' THEN 'UNIT'
        WHEN r.resource_type = 'feln' THEN 'EMOTION'
        ELSE 'PLAN'
    END,
    COALESCE(outp.interaction_date, r.created_at),
    r.created_at,
    r.updated_at
FROM resources r
LEFT JOIN LATERAL (
    SELECT i.interaction_user_id, i.interaction_date
    FROM interactions i
    WHERE i.resource_id = r.id
      AND i.interaction_type = 'outp'
    ORDER BY i.interaction_date DESC, i.created_at DESC
    LIMIT 1
) outp ON TRUE
JOIN LATERAL (
    SELECT rr.target_resource_id AS analysis_id, rr.user_id
    FROM resource_relations rr
    JOIN landscape_analyses l ON l.id = rr.target_resource_id
    WHERE rr.origin_resource_id = r.id
      AND rr.relation_type = 'ownr'
    ORDER BY rr.updated_at DESC, rr.created_at DESC
    LIMIT 1
) ownr_rel ON TRUE
JOIN LATERAL (
    SELECT rr.target_resource_id AS trace_id
    FROM resource_relations rr
    JOIN traces t ON t.id = rr.target_resource_id
    WHERE rr.origin_resource_id = r.id
      AND rr.relation_type = 'elmt'
    ORDER BY rr.updated_at DESC, rr.created_at DESC
    LIMIT 1
) trace_rel ON TRUE
LEFT JOIN LATERAL (
    SELECT rr.target_resource_id AS trace_mirror_id
    FROM resource_relations rr
    JOIN trace_mirrors tm ON tm.id = rr.target_resource_id
    WHERE rr.origin_resource_id = r.id
      AND rr.relation_type = 'trcm'
    ORDER BY rr.updated_at DESC, rr.created_at DESC
    LIMIT 1
) trcm_rel ON TRUE
WHERE r.entity_type = 'elmt'
  AND COALESCE(outp.interaction_user_id, ownr_rel.user_id) IS NOT NULL
ON CONFLICT (id) DO NOTHING;

WITH ref_source AS (
    SELECT
        rr.id,
        rr.origin_resource_id AS trace_mirror_id,
        rr.target_resource_id AS landmark_id,
        rr.user_id,
        rr.created_at,
        rr.updated_at,
        safe_jsonb(rr.relation_comment) AS payload
    FROM resource_relations rr
    JOIN trace_mirrors tm ON tm.id = rr.origin_resource_id
    JOIN landmarks lm ON lm.id = rr.target_resource_id
    WHERE rr.relation_type = 'rfrr'
)
INSERT INTO "references" (
    id,
    tag_id,
    trace_mirror_id,
    landmark_id,
    landscape_analysis_id,
    user_id,
    mention,
    reference_type,
    context_tags,
    reference_variants,
    parent_reference_id,
    is_user_specific,
    created_at,
    updated_at
)
SELECT
    rs.id,
    COALESCE(
        safe_int(rs.payload ->> 'tag_id'),
        safe_int(rs.payload ->> 'local_id'),
        (ROW_NUMBER() OVER (PARTITION BY rs.trace_mirror_id ORDER BY rs.created_at, rs.id) - 1)::INT
    ) AS tag_id,
    rs.trace_mirror_id,
    rs.landmark_id,
    COALESCE(safe_uuid(rs.payload ->> 'landscape_analysis_id'), tm.landscape_analysis_id),
    rs.user_id,
    COALESCE(NULLIF(rs.payload ->> 'mention', ''), lm.title),
    COALESCE(NULLIF(rs.payload ->> 'reference_type', ''), 'PLAIN_DESC'),
    CASE
        WHEN jsonb_typeof(rs.payload -> 'context_tags') = 'array' THEN rs.payload -> 'context_tags'
        ELSE '[]'::JSONB
    END,
    CASE
        WHEN jsonb_typeof(rs.payload -> 'reference_variants') = 'array' THEN rs.payload -> 'reference_variants'
        ELSE '[]'::JSONB
    END,
    NULL,
    COALESCE(safe_bool(rs.payload ->> 'is_user_specific'), FALSE),
    rs.created_at,
    rs.updated_at
FROM ref_source rs
JOIN trace_mirrors tm ON tm.id = rs.trace_mirror_id
JOIN landmarks lm ON lm.id = rs.landmark_id
WHERE rs.user_id IS NOT NULL
ON CONFLICT (id) DO NOTHING;

WITH ref_parent_source AS (
    SELECT
        rr.id AS ref_id,
        safe_uuid(safe_jsonb(rr.relation_comment) ->> 'parent_reference_id') AS parent_reference_id
    FROM resource_relations rr
    WHERE rr.relation_type = 'rfrr'
)
UPDATE "references" r
SET parent_reference_id = rps.parent_reference_id
FROM ref_parent_source rps
WHERE r.id = rps.ref_id
  AND rps.parent_reference_id IS NOT NULL
  AND EXISTS (SELECT 1 FROM "references" p WHERE p.id = rps.parent_reference_id);

INSERT INTO lenses (
    id,
    user_id,
    processing_state,
    fork_landscape_id,
    current_landscape_id,
    target_trace_id,
    autoplay,
    created_at,
    updated_at
)
SELECT
    r.id,
    COALESCE(outp.interaction_user_id, fork_rel.user_id, head_rel.user_id, target_rel.user_id),
    CASE
        WHEN r.maturing_state = 'fnsh' THEN 'IN_SYNC'
        ELSE 'OUT_OF_SYNC'
    END,
    fork_rel.fork_landscape_id,
    head_rel.current_landscape_id,
    target_rel.target_trace_id,
    COALESCE(r.is_external, FALSE),
    r.created_at,
    r.updated_at
FROM resources r
LEFT JOIN LATERAL (
    SELECT i.interaction_user_id
    FROM interactions i
    WHERE i.resource_id = r.id
      AND i.interaction_type = 'outp'
    ORDER BY i.interaction_date DESC, i.created_at DESC
    LIMIT 1
) outp ON TRUE
LEFT JOIN LATERAL (
    SELECT rr.target_resource_id AS fork_landscape_id, rr.user_id
    FROM resource_relations rr
    JOIN landscape_analyses l ON l.id = rr.target_resource_id
    WHERE rr.origin_resource_id = r.id
      AND rr.relation_type = 'fork'
    ORDER BY rr.updated_at DESC, rr.created_at DESC
    LIMIT 1
) fork_rel ON TRUE
LEFT JOIN LATERAL (
    SELECT rr.target_resource_id AS current_landscape_id, rr.user_id
    FROM resource_relations rr
    JOIN landscape_analyses l ON l.id = rr.target_resource_id
    WHERE rr.origin_resource_id = r.id
      AND rr.relation_type = 'head'
    ORDER BY rr.updated_at DESC, rr.created_at DESC
    LIMIT 1
) head_rel ON TRUE
LEFT JOIN LATERAL (
    SELECT rr.target_resource_id AS target_trace_id, rr.user_id
    FROM resource_relations rr
    JOIN traces t ON t.id = rr.target_resource_id
    WHERE rr.origin_resource_id = r.id
      AND rr.relation_type = 'trgt'
    ORDER BY rr.updated_at DESC, rr.created_at DESC
    LIMIT 1
) target_rel ON TRUE
WHERE r.entity_type = 'lens'
  AND COALESCE(outp.interaction_user_id, fork_rel.user_id, head_rel.user_id, target_rel.user_id) IS NOT NULL
ON CONFLICT (id) DO NOTHING;

INSERT INTO posts (
    id,
    user_id,
    title,
    subtitle,
    content,
    image_url,
    post_type,
    publishing_date,
    publishing_state,
    maturing_state,
    created_at,
    updated_at
)
SELECT
    i.id,
    i.interaction_user_id,
    COALESCE(r.title, ''),
    COALESCE(r.subtitle, ''),
    COALESCE(r.content, ''),
    r.image_url,
    CASE
        WHEN i.interaction_type = 'inpt' THEN 'INPUT'
        WHEN i.interaction_type = 'pblm' THEN 'PROBLEM'
        WHEN i.interaction_type = 'wish' THEN 'WISH'
        ELSE 'OUTPUT'
    END,
    i.interaction_date,
    COALESCE(r.publishing_state, 'pbsh'),
    COALESCE(r.maturing_state, 'drft'),
    COALESCE(i.created_at, r.created_at),
    GREATEST(COALESCE(i.updated_at, i.created_at), r.updated_at)
FROM interactions i
JOIN resources r ON r.id = i.resource_id
WHERE r.entity_type = 'ppst'
  AND i.interaction_type IN ('outp', 'inpt', 'pblm', 'wish')
ON CONFLICT (id) DO NOTHING;

-- ---------------------------------------------------------------------------
-- Backfill relation tables
-- ---------------------------------------------------------------------------

INSERT INTO element_relations (
    origin_element_id,
    target_element_id,
    relation_type,
    created_at,
    updated_at
)
SELECT
    rr.origin_resource_id,
    rr.target_resource_id,
    CASE
        WHEN rr.relation_type = 'applies_to' THEN 'APPLIES_TO'
        WHEN rr.relation_type = 'subtask_of' THEN 'SUBTASK_OF'
        ELSE 'THEME_OF'
    END,
    rr.created_at,
    rr.updated_at
FROM resource_relations rr
JOIN elements e1 ON e1.id = rr.origin_resource_id
JOIN elements e2 ON e2.id = rr.target_resource_id
WHERE rr.relation_type IN ('applies_to', 'subtask_of', 'theme_of')
ON CONFLICT (origin_element_id, target_element_id, relation_type) DO NOTHING;

INSERT INTO element_landmarks (
    element_id,
    landmark_id,
    created_at,
    updated_at
)
SELECT
    rr.origin_resource_id,
    rr.target_resource_id,
    rr.created_at,
    rr.updated_at
FROM resource_relations rr
JOIN elements e ON e.id = rr.origin_resource_id
JOIN landmarks l ON l.id = rr.target_resource_id
WHERE rr.relation_type = 'elmt'
ON CONFLICT (element_id, landmark_id) DO NOTHING;

INSERT INTO landmark_relations (
    origin_landmark_id,
    target_landmark_id,
    relation_type,
    created_at,
    updated_at
)
SELECT
    rr.origin_resource_id,
    rr.target_resource_id,
    CASE
        WHEN rr.relation_type = 'hlpr' THEN 'HIGH_LEVEL_PROJECT_RELATED_TO'
        ELSE 'CHILD_OF'
    END,
    rr.created_at,
    rr.updated_at
FROM resource_relations rr
JOIN landmarks l1 ON l1.id = rr.origin_resource_id
JOIN landmarks l2 ON l2.id = rr.target_resource_id
WHERE rr.relation_type IN ('prnt', 'hlpr')
ON CONFLICT (origin_landmark_id, target_landmark_id, relation_type) DO NOTHING;

INSERT INTO landscape_landmarks (
    landscape_analysis_id,
    landmark_id,
    relation_type,
    created_at,
    updated_at
)
SELECT
    rr.target_resource_id,
    rr.origin_resource_id,
    CASE
        WHEN rr.relation_type = 'refr' THEN 'REFERENCED'
        ELSE 'OWNED_BY_ANALYSIS'
    END,
    rr.created_at,
    rr.updated_at
FROM resource_relations rr
JOIN landmarks l ON l.id = rr.origin_resource_id
JOIN landscape_analyses la ON la.id = rr.target_resource_id
WHERE rr.relation_type IN ('ownr', 'refr')
ON CONFLICT (landscape_analysis_id, landmark_id, relation_type) DO NOTHING;

INSERT INTO lens_analysis_scopes (
    lens_id,
    landscape_analysis_id,
    created_at,
    updated_at
)
SELECT
    rr.origin_resource_id,
    rr.target_resource_id,
    rr.created_at,
    rr.updated_at
FROM resource_relations rr
JOIN lenses l ON l.id = rr.origin_resource_id
JOIN landscape_analyses la ON la.id = rr.target_resource_id
WHERE rr.relation_type = 'lnsa'
ON CONFLICT (lens_id, landscape_analysis_id) DO NOTHING;

INSERT INTO lens_heads (
    lens_id,
    landscape_analysis_id,
    created_at,
    updated_at
)
SELECT DISTINCT ON (rr.origin_resource_id)
    rr.origin_resource_id,
    rr.target_resource_id,
    rr.created_at,
    rr.updated_at
FROM resource_relations rr
JOIN lenses l ON l.id = rr.origin_resource_id
JOIN landscape_analyses la ON la.id = rr.target_resource_id
WHERE rr.relation_type = 'head'
ORDER BY rr.origin_resource_id, rr.updated_at DESC, rr.created_at DESC
ON CONFLICT (lens_id)
DO UPDATE SET
    landscape_analysis_id = EXCLUDED.landscape_analysis_id,
    updated_at = EXCLUDED.updated_at;

INSERT INTO lens_targets (
    lens_id,
    trace_id,
    created_at,
    updated_at
)
SELECT DISTINCT ON (rr.origin_resource_id)
    rr.origin_resource_id,
    rr.target_resource_id,
    rr.created_at,
    rr.updated_at
FROM resource_relations rr
JOIN lenses l ON l.id = rr.origin_resource_id
JOIN traces t ON t.id = rr.target_resource_id
WHERE rr.relation_type = 'trgt'
ORDER BY rr.origin_resource_id, rr.updated_at DESC, rr.created_at DESC
ON CONFLICT (lens_id)
DO UPDATE SET
    trace_id = EXCLUDED.trace_id,
    updated_at = EXCLUDED.updated_at;

WITH resource_post_map AS (
    SELECT DISTINCT ON (i.resource_id)
        i.resource_id,
        p.id AS post_id
    FROM interactions i
    JOIN posts p ON p.id = i.id
    WHERE i.resource_id IS NOT NULL
      AND i.interaction_type IN ('outp', 'inpt', 'pblm', 'wish')
    ORDER BY i.resource_id, i.interaction_date DESC, i.created_at DESC, i.id DESC
)
INSERT INTO post_relations (
    origin_post_id,
    target_post_id,
    relation_type,
    created_at,
    updated_at
)
SELECT
    opm.post_id,
    tpm.post_id,
    'BIBLIOGRAPHY',
    rr.created_at,
    rr.updated_at
FROM resource_relations rr
JOIN resource_post_map opm ON opm.resource_id = rr.origin_resource_id
JOIN resource_post_map tpm ON tpm.resource_id = rr.target_resource_id
WHERE rr.relation_type = 'bibl'
ON CONFLICT (origin_post_id, target_post_id, relation_type) DO NOTHING;

COMMIT;
