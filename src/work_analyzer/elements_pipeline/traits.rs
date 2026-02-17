use crate::entities_v2::landmark::NewLandmark;
use crate::work_analyzer::elements_pipeline::types::IdentityState;
use uuid::Uuid;

pub trait NewLandmarkForExtractedElement {
    fn title(&self) -> String;
    fn subtitle(&self) -> String;
    fn content(&self) -> String;
    fn identity_state(&self) -> IdentityState;
    fn landmark_type(&self) -> crate::entities_v2::landmark::LandmarkType;

    fn to_new_landmark(
        &self,
        analysis_id: Uuid,
        user_id: Uuid,
        parent_id: Option<Uuid>,
    ) -> NewLandmark {
        NewLandmark::new(
            self.title(),
            self.subtitle(),
            self.content(),
            self.landmark_type(),
            self.identity_state().into(),
            analysis_id,
            user_id,
            parent_id,
        )
    }
}
