ALTER TABLE wal_days
    ADD COLUMN input_revision BIGINT NOT NULL DEFAULT 0;

CREATE TABLE wal_entries (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    wal_day_id UUID NOT NULL REFERENCES wal_days(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CONSTRAINT wal_entries_unique_day_position UNIQUE (wal_day_id, position),
    CONSTRAINT wal_entries_content_not_empty CHECK (BTRIM(content) <> '')
);

CREATE INDEX wal_entries_wal_day_position_idx
    ON wal_entries (wal_day_id, position);

INSERT INTO wal_entries (id, wal_day_id, position, content, created_at)
SELECT uuid_generate_v4(), wd.id, 0, wd.input, wd.created_at
FROM wal_days wd
WHERE BTRIM(wd.input) <> '';

UPDATE wal_days
SET input_revision = CASE WHEN BTRIM(input) = '' THEN 0 ELSE 1 END;

CREATE TABLE wal_projections (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    wal_day_id UUID NOT NULL REFERENCES wal_days(id) ON DELETE CASCADE,
    projection_type TEXT NOT NULL,
    target_date DATE NULL,
    status TEXT NOT NULL DEFAULT 'PENDING',
    source_revision BIGINT NOT NULL,
    schema_version INTEGER NOT NULL DEFAULT 1,
    prompt_version TEXT NOT NULL,
    content JSONB NULL,
    error_message TEXT NULL,
    generated_at TIMESTAMP NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CONSTRAINT wal_projections_type_check CHECK (
        projection_type IN ('OPERATIONAL', 'THEMATIC', 'CARRYOVER')
    ),
    CONSTRAINT wal_projections_status_check CHECK (
        status IN ('PENDING', 'PROCESSING', 'READY', 'FAILED', 'SKIPPED_AI_DISABLED', 'STALE')
    ),
    CONSTRAINT wal_projections_target_date_check CHECK (
        (projection_type = 'CARRYOVER' AND target_date IS NOT NULL)
        OR
        (projection_type IN ('OPERATIONAL', 'THEMATIC') AND target_date IS NULL)
    )
);

CREATE UNIQUE INDEX wal_projections_current_compilation_idx
    ON wal_projections (wal_day_id, projection_type)
    WHERE target_date IS NULL;

CREATE UNIQUE INDEX wal_projections_carryover_target_idx
    ON wal_projections (wal_day_id, projection_type, target_date)
    WHERE target_date IS NOT NULL;

CREATE INDEX wal_projections_status_type_idx
    ON wal_projections (status, projection_type, target_date);

SELECT diesel_manage_updated_at('wal_projections');
