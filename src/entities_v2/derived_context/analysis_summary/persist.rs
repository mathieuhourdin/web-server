use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::schema::analysis_summaries;

use super::model::{AnalysisSummary, NewAnalysisSummary};

impl AnalysisSummary {
    pub fn update(self, pool: &DbPool) -> Result<AnalysisSummary, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        diesel::update(analysis_summaries::table.filter(analysis_summaries::id.eq(self.id)))
            .set((
                analysis_summaries::summary_type.eq(self.summary_type.to_db()),
                analysis_summaries::title.eq(self.title),
                analysis_summaries::content.eq(self.content),
            ))
            .execute(&mut conn)?;
        AnalysisSummary::find(self.id, pool)
    }
}

impl NewAnalysisSummary {
    pub fn create(self, pool: &DbPool) -> Result<AnalysisSummary, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let id: Uuid = diesel::insert_into(analysis_summaries::table)
            .values((
                analysis_summaries::landscape_analysis_id.eq(self.landscape_analysis_id),
                analysis_summaries::user_id.eq(self.user_id),
                analysis_summaries::summary_type.eq(self.summary_type.to_db()),
                analysis_summaries::title.eq(self.title),
                analysis_summaries::content.eq(self.content),
            ))
            .returning(analysis_summaries::id)
            .get_result(&mut conn)?;
        AnalysisSummary::find(id, pool)
    }
}
