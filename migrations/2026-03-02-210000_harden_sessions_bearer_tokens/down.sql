ALTER TABLE sessions
    DROP COLUMN IF EXISTS last_seen_at,
    DROP COLUMN IF EXISTS revoked_at,
    DROP COLUMN IF EXISTS secret_hash;
