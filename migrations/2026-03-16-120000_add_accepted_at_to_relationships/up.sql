ALTER TABLE relationships
ADD COLUMN accepted_at TIMESTAMP NULL;

UPDATE relationships
SET accepted_at = updated_at
WHERE status = 'ACCEPTED'
  AND accepted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_relationships_target_status_accepted_at
    ON relationships(target_user_id, status, accepted_at DESC, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_relationships_requester_status_accepted_at
    ON relationships(requester_user_id, status, accepted_at DESC, updated_at DESC);
