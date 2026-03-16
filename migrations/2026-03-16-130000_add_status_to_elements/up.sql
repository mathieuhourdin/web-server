ALTER TABLE elements
ADD COLUMN status TEXT NULL CHECK (status IN ('DONE', 'IN_PROGRESS', 'INTENDED'));

UPDATE elements
SET status = CASE
    WHEN element_type = 'TRANSACTION' THEN
        CASE
            WHEN substring(content from E'"status"[[:space:]]*:[[:space:]]*"([A-Z_]+)"')
                IN ('DONE', 'IN_PROGRESS', 'INTENDED')
                THEN substring(content from E'"status"[[:space:]]*:[[:space:]]*"([A-Z_]+)"')
            ELSE 'DONE'
        END
    ELSE NULL
END;

CREATE INDEX IF NOT EXISTS idx_elements_status_created_at
    ON elements(status, created_at DESC);
