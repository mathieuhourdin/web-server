UPDATE posts p
SET publishing_date = COALESCE(t.finalized_at, p.created_at)
FROM traces t
WHERE p.source_trace_id = t.id
  AND p.status = 'PUBLISHED'
  AND p.publishing_date IS NULL;
