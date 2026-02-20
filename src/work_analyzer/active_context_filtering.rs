use std::collections::HashSet;

use crate::entities::error::PpdcError;
use crate::entities_v2::{landmark::Landmark, landscape_analysis::LandscapeAnalysis};
use crate::work_analyzer::analysis_processor::AnalysisContext;

pub fn run(
    context: &AnalysisContext,
    current_landscape: &LandscapeAnalysis,
    previous_landscape_landmarks: &[Landmark],
    user_high_level_projects: &[Landmark],
) -> Result<Vec<Landmark>, PpdcError> {
    let mut linked_landmark_ids = current_landscape
        .get_landmarks(None, &context.pool)?
        .into_iter()
        .map(|landmark| landmark.id)
        .collect::<HashSet<_>>();
    let mut linked_landmarks = Vec::new();

    for landmark in previous_landscape_landmarks
        .iter()
        .chain(user_high_level_projects.iter())
    {
        if linked_landmark_ids.insert(landmark.id) {
            crate::entities_v2::landscape_analysis::add_landmark_ref(
                context.analysis_id,
                landmark.id,
                context.user_id,
                &context.pool,
            )?;
            linked_landmarks.push(landmark.clone());
        }
    }

    Ok(linked_landmarks)
}
