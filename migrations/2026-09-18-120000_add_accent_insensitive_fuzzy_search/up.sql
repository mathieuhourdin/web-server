CREATE EXTENSION IF NOT EXISTS unaccent;
CREATE EXTENSION IF NOT EXISTS pg_trgm;

UPDATE trace_search_documents
SET search_vector =
    setweight(to_tsvector('simple', unaccent(title)), 'A') ||
    setweight(to_tsvector('simple', unaccent(content)), 'A') ||
    setweight(to_tsvector('simple', unaccent(tag_text)), 'A') ||
    setweight(to_tsvector('simple', unaccent(mirror_text)), 'B') ||
    setweight(to_tsvector('simple', unaccent(element_text)), 'C') ||
    setweight(to_tsvector('simple', unaccent(reference_text)), 'C') ||
    setweight(to_tsvector('simple', unaccent(landmark_text)), 'C') ||
    setweight(to_tsvector('simple', unaccent(high_level_project_landmark_text)), 'C');
