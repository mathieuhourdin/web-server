use std::collections::HashSet;

use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::entities_v2::{
    landmark::{Landmark, LandmarkType},
    landscape_analysis::LandscapeAnalysis,
};

pub struct AnalysisContext {
    pub analysis_id: Uuid,
    pub user_id: Uuid,
    pub pool: DbPool,
}

pub fn load_previous_landscape_inputs(
    previous_landscape_id: Option<Uuid>,
    pool: &DbPool,
) -> Result<(Option<LandscapeAnalysis>, Vec<Landmark>, Vec<Landmark>), PpdcError> {
    match previous_landscape_id {
        Some(previous_landscape_id) => {
            let previous_landscape =
                LandscapeAnalysis::find_full_analysis(previous_landscape_id, pool)?;
            let mut seen_hlp_ids = HashSet::new();
            let mut seen_non_hlp_ids = HashSet::new();

            let all_previous_landscape_landmarks = previous_landscape.get_landmarks(None, pool)?;
            let user_high_level_projects = all_previous_landscape_landmarks
                .iter()
                .filter(|landmark| landmark.landmark_type == LandmarkType::HighLevelProject)
                .filter(|landmark| seen_hlp_ids.insert(landmark.id))
                .cloned()
                .collect::<Vec<_>>();

            let previous_landscape_landmarks = all_previous_landscape_landmarks
                .into_iter()
                .filter(|landmark| landmark.landmark_type != LandmarkType::HighLevelProject)
                .filter(|landmark| seen_non_hlp_ids.insert(landmark.id))
                .collect::<Vec<_>>();
            Ok((
                Some(previous_landscape),
                previous_landscape_landmarks,
                user_high_level_projects,
            ))
        }
        None => Ok((None, vec![], vec![])),
    }
}
