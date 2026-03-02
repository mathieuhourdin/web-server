ALTER TABLE sessions
    ADD COLUMN secret_hash TEXT,
    ADD COLUMN revoked_at TIMESTAMP,
    ADD COLUMN last_seen_at TIMESTAMP;
