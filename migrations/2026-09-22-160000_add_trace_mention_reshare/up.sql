ALTER TABLE trace_mentions
    ADD COLUMN allows_reshare BOOLEAN NOT NULL DEFAULT FALSE;

DROP INDEX IF EXISTS trace_mentions_granted_access_idx;

ALTER TABLE trace_mentions
    DROP COLUMN grants_trace_access;

CREATE INDEX trace_mentions_active_idx
    ON trace_mentions (mentioned_user_id, trace_id)
    WHERE removed_at IS NULL;

CREATE INDEX trace_mentions_active_reshare_idx
    ON trace_mentions (mentioned_user_id, trace_id)
    WHERE removed_at IS NULL AND allows_reshare = TRUE;
