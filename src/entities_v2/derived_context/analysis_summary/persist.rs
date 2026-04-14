use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::schema::analysis_summaries;

use super::model::{AnalysisSummary, NewAnalysisSummary};

impl AnalysisSummary {
    pub fn update(self, pool: &DbPool) -> Result<AnalysisSummary, PpdcError> {
        let mut conn = pool
            .get()?;
        let meaningful_event_title = self
            .meaningful_event
            .as_ref()
            .map(|event| event.title.clone());
        let meaningful_event_description = self
            .meaningful_event
            .as_ref()
            .map(|event| event.description.clone());
        let meaningful_event_date = self
            .meaningful_event
            .as_ref()
            .map(|event| event.event_date.clone());
        diesel::update(analysis_summaries::table.filter(analysis_summaries::id.eq(self.id)))
            .set((
                analysis_summaries::summary_type.eq(self.summary_type.to_db()),
                analysis_summaries::title.eq(self.title),
                analysis_summaries::short_content.eq(self.short_content),
                analysis_summaries::content.eq(self.content),
                analysis_summaries::meaningful_event_title.eq(meaningful_event_title),
                analysis_summaries::meaningful_event_description.eq(meaningful_event_description),
                analysis_summaries::meaningful_event_date.eq(meaningful_event_date),
            ))
            .execute(&mut conn)?;
        AnalysisSummary::find(self.id, pool)
    }
}

impl NewAnalysisSummary {
    pub fn create(self, pool: &DbPool) -> Result<AnalysisSummary, PpdcError> {
        let mut conn = pool
            .get()?;
        let meaningful_event_title = self
            .meaningful_event
            .as_ref()
            .map(|event| event.title.clone());
        let meaningful_event_description = self
            .meaningful_event
            .as_ref()
            .map(|event| event.description.clone());
        let meaningful_event_date = self
            .meaningful_event
            .as_ref()
            .map(|event| event.event_date.clone());
        let id: Uuid = diesel::insert_into(analysis_summaries::table)
            .values((
                analysis_summaries::landscape_analysis_id.eq(self.landscape_analysis_id),
                analysis_summaries::user_id.eq(self.user_id),
                analysis_summaries::summary_type.eq(self.summary_type.to_db()),
                analysis_summaries::title.eq(self.title),
                analysis_summaries::short_content.eq(self.short_content),
                analysis_summaries::content.eq(self.content),
                analysis_summaries::meaningful_event_title.eq(meaningful_event_title),
                analysis_summaries::meaningful_event_description.eq(meaningful_event_description),
                analysis_summaries::meaningful_event_date.eq(meaningful_event_date),
            ))
            .returning(analysis_summaries::id)
            .get_result(&mut conn)?;
        AnalysisSummary::find(id, pool)
    }
}
