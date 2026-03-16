DROP INDEX IF EXISTS idx_landmarks_last_related_element_at;
DROP INDEX IF EXISTS idx_landmarks_related_elements_count;

ALTER TABLE landmarks
DROP COLUMN IF EXISTS last_related_element_at,
DROP COLUMN IF EXISTS related_elements_count;
