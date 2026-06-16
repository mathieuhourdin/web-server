CREATE INDEX IF NOT EXISTS idx_landscape_landmarks_landmark_analysis
ON landscape_landmarks (landmark_id, landscape_analysis_id);

CREATE INDEX IF NOT EXISTS idx_element_landmarks_landmark_element
ON element_landmarks (landmark_id, element_id);

CREATE INDEX IF NOT EXISTS idx_trace_mirrors_primary_landmark_trace
ON trace_mirrors (primary_landmark_id, trace_id)
WHERE primary_landmark_id IS NOT NULL;
