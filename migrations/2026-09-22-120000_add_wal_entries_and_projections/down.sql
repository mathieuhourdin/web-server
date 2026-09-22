DROP TABLE IF EXISTS wal_projections;
DROP TABLE IF EXISTS wal_entries;

ALTER TABLE wal_days
    DROP COLUMN IF EXISTS input_revision;
