DROP INDEX IF EXISTS trace_mentions_active_reshare_idx;
DROP INDEX IF EXISTS trace_mentions_active_idx;

ALTER TABLE trace_mentions
    ADD COLUMN grants_trace_access BOOLEAN NOT NULL DEFAULT FALSE;

UPDATE trace_mentions
SET grants_trace_access = TRUE
WHERE removed_at IS NULL;

CREATE INDEX trace_mentions_granted_access_idx
    ON trace_mentions (mentioned_user_id, trace_id)
    WHERE removed_at IS NULL AND grants_trace_access = TRUE;

ALTER TABLE trace_mentions
    DROP COLUMN IF EXISTS allows_reshare;
