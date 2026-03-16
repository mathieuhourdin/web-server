ALTER TABLE landmarks
ADD COLUMN related_elements_count INTEGER NOT NULL DEFAULT 0,
ADD COLUMN last_related_element_at TIMESTAMP NULL;

UPDATE landmarks l
SET related_elements_count = stats.related_elements_count,
    last_related_element_at = stats.last_related_element_at
FROM (
    SELECT
        el.landmark_id,
        COUNT(*)::INT AS related_elements_count,
        MAX(e.interaction_date) AS last_related_element_at
    FROM element_landmarks el
    INNER JOIN elements e ON e.id = el.element_id
    GROUP BY el.landmark_id
) AS stats
WHERE l.id = stats.landmark_id;

CREATE INDEX IF NOT EXISTS idx_landmarks_related_elements_count
    ON landmarks(related_elements_count DESC, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_landmarks_last_related_element_at
    ON landmarks(last_related_element_at DESC, updated_at DESC);
