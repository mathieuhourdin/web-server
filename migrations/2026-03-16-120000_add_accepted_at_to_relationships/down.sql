DROP INDEX IF EXISTS idx_relationships_requester_status_accepted_at;
DROP INDEX IF EXISTS idx_relationships_target_status_accepted_at;

ALTER TABLE relationships
DROP COLUMN IF EXISTS accepted_at;
