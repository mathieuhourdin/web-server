UPDATE trace_search_documents
SET search_vector =
    setweight(to_tsvector('simple', title), 'A') ||
    setweight(to_tsvector('simple', content), 'A') ||
    setweight(to_tsvector('simple', tag_text), 'A') ||
    setweight(to_tsvector('simple', mirror_text), 'B') ||
    setweight(to_tsvector('simple', element_text), 'C') ||
    setweight(to_tsvector('simple', reference_text), 'C') ||
    setweight(to_tsvector('simple', landmark_text), 'C') ||
    setweight(to_tsvector('simple', high_level_project_landmark_text), 'C');

DROP EXTENSION IF EXISTS unaccent;
