CREATE TABLE IF NOT EXISTS trace_search_documents (
    trace_id UUID PRIMARY KEY REFERENCES traces(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    journal_id UUID NOT NULL REFERENCES journals(id) ON DELETE CASCADE,
    interaction_date TIMESTAMP NOT NULL,
    title TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL DEFAULT '',
    mirror_text TEXT NOT NULL DEFAULT '',
    tag_text TEXT NOT NULL DEFAULT '',
    element_text TEXT NOT NULL DEFAULT '',
    landmark_text TEXT NOT NULL DEFAULT '',
    search_vector TSVECTOR NOT NULL,
    refreshed_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_trace_search_documents_vector
ON trace_search_documents
USING GIN (search_vector);

CREATE INDEX IF NOT EXISTS idx_trace_search_documents_user_interaction_date
ON trace_search_documents (user_id, interaction_date DESC);

INSERT INTO trace_search_documents (
    trace_id,
    user_id,
    journal_id,
    interaction_date,
    title,
    content,
    mirror_text,
    tag_text,
    element_text,
    landmark_text,
    search_vector,
    refreshed_at
)
SELECT
    doc.trace_id,
    doc.user_id,
    doc.journal_id,
    doc.interaction_date,
    doc.title,
    doc.content,
    doc.mirror_text,
    doc.tag_text,
    doc.element_text,
    doc.landmark_text,
    setweight(to_tsvector('simple', doc.title), 'A') ||
    setweight(to_tsvector('simple', doc.content), 'A') ||
    setweight(to_tsvector('simple', doc.tag_text), 'A') ||
    setweight(to_tsvector('simple', doc.mirror_text), 'B') ||
    setweight(to_tsvector('simple', doc.element_text), 'C') ||
    setweight(to_tsvector('simple', doc.landmark_text), 'C'),
    NOW()
FROM (
    SELECT
        t.id AS trace_id,
        t.user_id,
        t.journal_id,
        t.interaction_date,
        t.title,
        t.content,
        COALESCE(mp.mirror_text, '') AS mirror_text,
        COALESCE(mp.tag_text, '') AS tag_text,
        COALESCE(ep.element_text, '') AS element_text,
        CONCAT_WS(
            ' ',
            COALESCE(lp.landmark_text, ''),
            COALESCE(plp.landmark_text, ''),
            COALESCE(dlp.landmark_text, '')
        ) AS landmark_text
    FROM traces t
    LEFT JOIN LATERAL (
        SELECT
            COALESCE(string_agg(CONCAT_WS(' ', tm.title, tm.subtitle), ' '), '') AS mirror_text,
            COALESCE(string_agg(tag_value, ' '), '') AS tag_text
        FROM trace_mirrors tm
        INNER JOIN users u
          ON u.id = t.user_id
        INNER JOIN lens_analysis_scopes las
          ON las.landscape_analysis_id = tm.landscape_analysis_id
         AND las.lens_id = u.current_lens_id
        LEFT JOIN LATERAL jsonb_array_elements_text(tm.tags) AS tag_value ON TRUE
        WHERE tm.trace_id = t.id
    ) mp ON TRUE
    LEFT JOIN LATERAL (
        SELECT COALESCE(string_agg(CONCAT_WS(' ', e.title, e.subtitle, e.verb), ' '), '') AS element_text
        FROM elements e
        INNER JOIN users u
          ON u.id = t.user_id
        INNER JOIN lens_analysis_scopes las
          ON las.landscape_analysis_id = e.analysis_id
         AND las.lens_id = u.current_lens_id
        WHERE e.trace_id = t.id
    ) ep ON TRUE
    LEFT JOIN LATERAL (
        SELECT COALESCE(string_agg(DISTINCT CONCAT_WS(' ', l.title, l.subtitle), ' '), '') AS landmark_text
        FROM elements e
        INNER JOIN users u
          ON u.id = t.user_id
        INNER JOIN lens_analysis_scopes las
          ON las.landscape_analysis_id = e.analysis_id
         AND las.lens_id = u.current_lens_id
        JOIN element_landmarks el
          ON el.element_id = e.id
        JOIN landmarks l
          ON l.id = el.landmark_id
        WHERE e.trace_id = t.id
    ) lp ON TRUE
    LEFT JOIN LATERAL (
        SELECT COALESCE(string_agg(DISTINCT CONCAT_WS(' ', l.title, l.subtitle), ' '), '') AS landmark_text
        FROM trace_mirrors tm
        INNER JOIN users u
          ON u.id = t.user_id
        INNER JOIN lens_analysis_scopes las
          ON las.landscape_analysis_id = tm.landscape_analysis_id
         AND las.lens_id = u.current_lens_id
        JOIN landmarks l
          ON l.id = tm.primary_landmark_id
        WHERE tm.trace_id = t.id
    ) plp ON TRUE
    LEFT JOIN LATERAL (
        SELECT COALESCE(string_agg(DISTINCT CONCAT_WS(' ', l.title, l.subtitle), ' '), '') AS landmark_text
        FROM landscape_analyses la
        INNER JOIN users u
          ON u.id = t.user_id
        INNER JOIN lens_analysis_scopes las
          ON las.landscape_analysis_id = la.id
         AND las.lens_id = u.current_lens_id
        INNER JOIN landscape_landmarks ll
          ON ll.landscape_analysis_id = la.id
        INNER JOIN landmarks l
          ON l.id = ll.landmark_id
        WHERE la.analyzed_trace_id = t.id
    ) dlp ON TRUE
    WHERE t.status = 'FINALIZED'
) doc
ON CONFLICT (trace_id) DO UPDATE
SET user_id = EXCLUDED.user_id,
    journal_id = EXCLUDED.journal_id,
    interaction_date = EXCLUDED.interaction_date,
    title = EXCLUDED.title,
    content = EXCLUDED.content,
    mirror_text = EXCLUDED.mirror_text,
    tag_text = EXCLUDED.tag_text,
    element_text = EXCLUDED.element_text,
    landmark_text = EXCLUDED.landmark_text,
    search_vector = EXCLUDED.search_vector,
    refreshed_at = NOW();
