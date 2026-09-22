ALTER TABLE wal_days
    ADD COLUMN compilation_due_at TIMESTAMP NULL,
    ADD COLUMN compilation_started_at TIMESTAMP NULL,
    ADD COLUMN compilation_processing_revision BIGINT NULL,
    ADD COLUMN compilation_last_error TEXT NULL,
    ADD CONSTRAINT wal_days_compilation_claim_check CHECK (
        (compilation_started_at IS NULL AND compilation_processing_revision IS NULL)
        OR
        (compilation_started_at IS NOT NULL AND compilation_processing_revision IS NOT NULL)
    );

CREATE INDEX wal_days_compilation_due_idx
    ON wal_days (compilation_due_at)
    WHERE compilation_due_at IS NOT NULL;

CREATE INDEX wal_days_compilation_started_idx
    ON wal_days (compilation_started_at)
    WHERE compilation_started_at IS NOT NULL;
