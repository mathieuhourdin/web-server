WITH ranked_feedbacks AS (
    SELECT
        id,
        ROW_NUMBER() OVER (
            PARTITION BY landscape_analysis_id
            ORDER BY created_at DESC, updated_at DESC, id DESC
        ) AS row_number
    FROM messages
    WHERE message_type = 'MENTOR_FEEDBACK'
      AND landscape_analysis_id IS NOT NULL
)
DELETE FROM messages
WHERE id IN (
    SELECT id
    FROM ranked_feedbacks
    WHERE row_number > 1
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_unique_mentor_feedback_per_analysis
ON messages (landscape_analysis_id)
WHERE message_type = 'MENTOR_FEEDBACK';
