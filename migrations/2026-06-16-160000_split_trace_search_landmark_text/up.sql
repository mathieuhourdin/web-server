ALTER TABLE trace_search_documents
ADD COLUMN IF NOT EXISTS high_level_project_landmark_text TEXT NOT NULL DEFAULT '';

WITH refreshed AS (
    SELECT
        tsd.trace_id,
        COALESCE(mp.mirror_text, '') AS mirror_text,
        COALESCE(mp.tag_text, '') AS tag_text,
        COALESCE(ep.element_text, '') AS element_text,
        COALESCE(rp.reference_text, '') AS reference_text,
        COALESCE(rlp.landmark_text, '') AS landmark_text,
        COALESCE(hlp.high_level_project_landmark_text, '') AS high_level_project_landmark_text
    FROM trace_search_documents tsd
    LEFT JOIN LATERAL (
        SELECT
            COALESCE(string_agg(CONCAT_WS(' ', tm.title, tm.subtitle), ' '), '') AS mirror_text,
            COALESCE(string_agg(tag_value, ' '), '') AS tag_text
        FROM trace_mirrors tm
        INNER JOIN landscape_analyses la
          ON la.id = tm.landscape_analysis_id
         AND la.landscape_analysis_type = 'TRACE_INCREMENTAL'
        INNER JOIN users u
          ON u.id = tsd.user_id
        INNER JOIN lens_analysis_scopes las
          ON las.landscape_analysis_id = tm.landscape_analysis_id
         AND las.lens_id = u.current_lens_id
        LEFT JOIN LATERAL jsonb_array_elements_text(tm.tags) AS tag_value ON TRUE
        WHERE tm.trace_id = tsd.trace_id
    ) mp ON TRUE
    LEFT JOIN LATERAL (
        SELECT COALESCE(string_agg(CONCAT_WS(' ', e.title, e.subtitle, e.verb), ' '), '') AS element_text
        FROM elements e
        INNER JOIN landscape_analyses la
          ON la.id = e.analysis_id
         AND la.landscape_analysis_type = 'TRACE_INCREMENTAL'
        INNER JOIN users u
          ON u.id = tsd.user_id
        INNER JOIN lens_analysis_scopes las
          ON las.landscape_analysis_id = e.analysis_id
         AND las.lens_id = u.current_lens_id
        WHERE e.trace_id = tsd.trace_id
    ) ep ON TRUE
    LEFT JOIN LATERAL (
        SELECT COALESCE(string_agg(CONCAT_WS(' ', r.mention, tags.context_tags, variants.reference_variants), ' '), '') AS reference_text
        FROM "references" r
        INNER JOIN trace_mirrors tm
          ON tm.id = r.trace_mirror_id
        INNER JOIN landscape_analyses la
          ON la.id = r.landscape_analysis_id
         AND la.landscape_analysis_type = 'TRACE_INCREMENTAL'
        INNER JOIN users u
          ON u.id = tsd.user_id
        INNER JOIN lens_analysis_scopes las
          ON las.landscape_analysis_id = r.landscape_analysis_id
         AND las.lens_id = u.current_lens_id
        LEFT JOIN landmarks l
          ON l.id = r.landmark_id
        LEFT JOIN LATERAL (
            SELECT COALESCE(string_agg(value, ' '), '') AS context_tags
            FROM jsonb_array_elements_text(r.context_tags) AS tag_value(value)
        ) tags ON TRUE
        LEFT JOIN LATERAL (
            SELECT COALESCE(string_agg(value, ' '), '') AS reference_variants
            FROM jsonb_array_elements_text(r.reference_variants) AS variant_value(value)
        ) variants ON TRUE
        WHERE tm.trace_id = tsd.trace_id
          AND (l.id IS NULL OR l.landmark_type <> 'HIGH_LEVEL_PROJECT')
    ) rp ON TRUE
    LEFT JOIN LATERAL (
        SELECT COALESCE(string_agg(DISTINCT CONCAT_WS(' ', l.title, l.subtitle), ' '), '') AS landmark_text
        FROM "references" r
        INNER JOIN trace_mirrors tm
          ON tm.id = r.trace_mirror_id
        INNER JOIN landscape_analyses la
          ON la.id = r.landscape_analysis_id
         AND la.landscape_analysis_type = 'TRACE_INCREMENTAL'
        INNER JOIN users u
          ON u.id = tsd.user_id
        INNER JOIN lens_analysis_scopes las
          ON las.landscape_analysis_id = r.landscape_analysis_id
         AND las.lens_id = u.current_lens_id
        JOIN landmarks l
          ON l.id = r.landmark_id
        WHERE tm.trace_id = tsd.trace_id
          AND r.landmark_id IS NOT NULL
          AND l.landmark_type <> 'HIGH_LEVEL_PROJECT'
    ) rlp ON TRUE
    LEFT JOIN LATERAL (
        SELECT COALESCE(string_agg(DISTINCT CONCAT_WS(' ', l.title, l.subtitle), ' '), '') AS high_level_project_landmark_text
        FROM "references" r
        INNER JOIN trace_mirrors tm
          ON tm.id = r.trace_mirror_id
        INNER JOIN landscape_analyses la
          ON la.id = r.landscape_analysis_id
         AND la.landscape_analysis_type = 'TRACE_INCREMENTAL'
        INNER JOIN users u
          ON u.id = tsd.user_id
        INNER JOIN lens_analysis_scopes las
          ON las.landscape_analysis_id = r.landscape_analysis_id
         AND las.lens_id = u.current_lens_id
        JOIN landmarks l
          ON l.id = r.landmark_id
        WHERE tm.trace_id = tsd.trace_id
          AND r.landmark_id IS NOT NULL
          AND l.landmark_type = 'HIGH_LEVEL_PROJECT'
    ) hlp ON TRUE
)
UPDATE trace_search_documents tsd
SET
    mirror_text = refreshed.mirror_text,
    tag_text = refreshed.tag_text,
    element_text = refreshed.element_text,
    reference_text = refreshed.reference_text,
    landmark_text = refreshed.landmark_text,
    high_level_project_landmark_text = refreshed.high_level_project_landmark_text,
    search_vector =
        setweight(to_tsvector('simple', tsd.title), 'A') ||
        setweight(to_tsvector('simple', tsd.content), 'A') ||
        setweight(to_tsvector('simple', refreshed.tag_text), 'A') ||
        setweight(to_tsvector('simple', refreshed.mirror_text), 'B') ||
        setweight(to_tsvector('simple', refreshed.element_text), 'C') ||
        setweight(to_tsvector('simple', refreshed.reference_text), 'C') ||
        setweight(to_tsvector('simple', refreshed.landmark_text), 'C') ||
        setweight(to_tsvector('simple', refreshed.high_level_project_landmark_text), 'C'),
    refreshed_at = NOW()
FROM refreshed
WHERE refreshed.trace_id = tsd.trace_id;
