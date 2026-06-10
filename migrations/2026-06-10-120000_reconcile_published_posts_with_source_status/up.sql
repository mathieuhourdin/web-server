-- Forward-only reconciliation for the publication invariant (doc/publication.md).
--
-- The invariant (source no longer eligible => its post is archived) is now
-- enforced going forward in application code, but historical rows predate the
-- cascade: before today, archiving a document or album left its post published.
-- Those stale rows were masked by a read-time eligibility check in the feed,
-- which we are about to remove in favour of trusting posts.status alone.
--
-- Archive every published, source-backed post whose linked source record is not
-- currently eligible for publication. Source-less ("custom") posts are left
-- untouched: they are legitimately published and simply do not appear in the feed.
UPDATE posts
SET status = 'ARCHIVED',
    updated_at = NOW()
WHERE status = 'PUBLISHED'
  AND (
        (source_trace_id IS NOT NULL
         AND EXISTS (SELECT 1 FROM traces t
                     WHERE t.id = posts.source_trace_id
                       AND t.status <> 'FINALIZED'))
     OR (source_document_id IS NOT NULL
         AND EXISTS (SELECT 1 FROM documents d
                     WHERE d.id = posts.source_document_id
                       AND d.status = 'ARCHIVED'))
     OR (source_album_id IS NOT NULL
         AND EXISTS (SELECT 1 FROM albums a
                     WHERE a.id = posts.source_album_id
                       AND a.completion_status = 'ARCHIVED'))
  );
