DELETE FROM post_grants
WHERE post_id IN (
    SELECT p.id
    FROM posts p
    WHERE p.source_trace_id IS NOT NULL
      AND p.publishing_state = 'pbsh'
      AND p.post_type = 'IDEA'
      AND p.interaction_type = 'OUTPUT'
);

DELETE FROM posts
WHERE source_trace_id IS NOT NULL
  AND publishing_state = 'pbsh'
  AND post_type = 'IDEA'
  AND interaction_type = 'OUTPUT';
