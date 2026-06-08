ALTER TABLE journals
ADD COLUMN IF NOT EXISTS current_draft_id UUID NULL;

ALTER TABLE traces
ADD COLUMN IF NOT EXISTS is_blank BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE journals
DROP CONSTRAINT IF EXISTS journals_status_check;

ALTER TABLE journals
DROP CONSTRAINT IF EXISTS journals_journal_type_check;

UPDATE journals
SET status = CASE
    WHEN status = 'ARCHIVED' THEN 'ARCHIVED'
    ELSE 'ACTIVE'
END;

UPDATE journals
SET journal_type = CASE
    WHEN journal_type = 'META_JOURNAL' THEN 'META_JOURNAL'
    ELSE 'USER_JOURNAL'
END;

ALTER TABLE journals
ADD CONSTRAINT journals_status_check
CHECK (status IN ('ACTIVE', 'ARCHIVED'));

ALTER TABLE journals
ADD CONSTRAINT journals_journal_type_check
CHECK (journal_type IN ('META_JOURNAL', 'USER_JOURNAL'));

UPDATE traces t
SET is_blank = (
    NULLIF(BTRIM(t.title), '') IS NULL
    AND NULLIF(BTRIM(t.content), '') IS NULL
    AND t.content_image_asset_id IS NULL
    AND NOT EXISTS (
        SELECT 1
        FROM trace_attachments ta
        WHERE ta.trace_id = t.id
    )
);

WITH ranked_drafts AS (
    SELECT
        t.id,
        t.journal_id,
        ROW_NUMBER() OVER (
            PARTITION BY t.journal_id
            ORDER BY t.updated_at DESC, t.created_at DESC, t.id DESC
        ) AS rn
    FROM traces t
    JOIN journals j
      ON j.id = t.journal_id
    WHERE t.status = 'DRAFT'
      AND t.trace_type = 'USER_TRACE'
      AND j.journal_type = 'USER_JOURNAL'
),
non_canonical AS (
    SELECT id
    FROM ranked_drafts
    WHERE rn > 1
)
UPDATE traces
SET status = 'ARCHIVED',
    finalized_at = COALESCE(finalized_at, NOW()),
    updated_at = NOW()
WHERE id IN (SELECT id FROM non_canonical);

WITH eligible_journals AS (
    SELECT j.id AS journal_id, j.user_id
    FROM journals j
    WHERE j.status = 'ACTIVE'
      AND j.journal_type = 'USER_JOURNAL'
),
existing_drafts AS (
    SELECT DISTINCT ON (t.journal_id)
        t.journal_id,
        t.id AS trace_id
    FROM traces t
    JOIN eligible_journals ej
      ON ej.journal_id = t.journal_id
    WHERE t.status = 'DRAFT'
      AND t.trace_type = 'USER_TRACE'
    ORDER BY t.journal_id, t.updated_at DESC, t.created_at DESC, t.id DESC
),
inserted_drafts AS (
    INSERT INTO traces (
        id,
        user_id,
        journal_id,
        title,
        subtitle,
        content,
        interaction_date,
        trace_type,
        status,
        is_encrypted,
        encryption_metadata,
        start_writing_at,
        finalized_at,
        content_image_asset_id,
        timeout_at,
        timeout_start_at,
        sharing_sensitivity,
        derived_from_trace_id,
        is_blank,
        created_at,
        updated_at
    )
    SELECT
        uuid_generate_v4(),
        ej.user_id,
        ej.journal_id,
        '',
        '',
        '',
        NOW(),
        'USER_TRACE',
        'DRAFT',
        FALSE,
        NULL,
        NOW(),
        NULL,
        NULL,
        NULL,
        NULL,
        'NORMAL',
        NULL,
        TRUE,
        NOW(),
        NOW()
    FROM eligible_journals ej
    LEFT JOIN existing_drafts ed
      ON ed.journal_id = ej.journal_id
    WHERE ed.trace_id IS NULL
    RETURNING id, journal_id
),
all_current_drafts AS (
    SELECT journal_id, trace_id
    FROM existing_drafts
    UNION ALL
    SELECT journal_id, id AS trace_id
    FROM inserted_drafts
)
UPDATE journals j
SET current_draft_id = acd.trace_id,
    updated_at = NOW()
FROM all_current_drafts acd
WHERE j.id = acd.journal_id;

ALTER TABLE journals
ADD CONSTRAINT journals_active_user_journal_current_draft_check
CHECK (
    status <> 'ACTIVE'
    OR journal_type <> 'USER_JOURNAL'
    OR current_draft_id IS NOT NULL
);

ALTER TABLE journals
ADD CONSTRAINT journals_current_draft_id_fkey
FOREIGN KEY (current_draft_id)
REFERENCES traces(id)
ON DELETE SET NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_traces_unique_user_draft_per_journal
ON traces (journal_id)
WHERE status = 'DRAFT' AND trace_type = 'USER_TRACE';

CREATE INDEX IF NOT EXISTS idx_journals_current_draft_id
ON journals (current_draft_id);
