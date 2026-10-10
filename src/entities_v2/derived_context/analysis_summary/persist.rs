use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::schema::analysis_summaries;

use super::model::{AnalysisSummary, NewAnalysisSummary, SummaryEmojis};

impl AnalysisSummary {
    pub fn update(self, pool: &DbPool) -> Result<AnalysisSummary, PpdcError> {
        let mut conn = pool.get()?;
        let emojis = SummaryEmojis {
            activity_emoji: self.activity_emoji,
            mood_emoji: self.mood_emoji,
            dominant_emoji_kind: self.dominant_emoji_kind,
        }
        .normalized();
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
                analysis_summaries::activity_emoji.eq(emojis.activity_emoji),
                analysis_summaries::mood_emoji.eq(emojis.mood_emoji),
                analysis_summaries::dominant_emoji_kind
                    .eq(emojis.dominant_emoji_kind.map(|kind| kind.to_db())),
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
        let mut conn = pool.get()?;
        let emojis = self.emojis.normalized();
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
                analysis_summaries::activity_emoji.eq(emojis.activity_emoji),
                analysis_summaries::mood_emoji.eq(emojis.mood_emoji),
                analysis_summaries::dominant_emoji_kind
                    .eq(emojis.dominant_emoji_kind.map(|kind| kind.to_db())),
                analysis_summaries::meaningful_event_title.eq(meaningful_event_title),
                analysis_summaries::meaningful_event_description.eq(meaningful_event_description),
                analysis_summaries::meaningful_event_date.eq(meaningful_event_date),
            ))
            .returning(analysis_summaries::id)
            .get_result(&mut conn)?;
        AnalysisSummary::find(id, pool)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities_v2::analysis_summary::{NewAnalysisSummaryDto, SummaryEmojiKind};
    use diesel::r2d2::ConnectionManager;

    #[derive(QueryableByName)]
    struct AnalysisRow {
        #[diesel(sql_type = diesel::sql_types::Uuid)]
        id: Uuid,
        #[diesel(sql_type = diesel::sql_types::Uuid)]
        user_id: Uuid,
    }
    #[derive(QueryableByName)]
    struct StoredKind {
        #[diesel(sql_type = diesel::sql_types::Text)]
        kind: String,
    }

    #[test]
    #[ignore = "Requires an explicitly configured disposable summary emoji database"]
    fn summary_emoji_database_roundtrip() {
        let url = std::env::var("SUMMARY_EMOJI_TEST_DATABASE_URL")
            .expect("Set disposable emoji test URL");
        assert_eq!(
            url::Url::parse(&url).unwrap().path(),
            "/hupo_summary_emoji_test_20261010"
        );
        let pool = DbPool::builder()
            .max_size(2)
            .build(ConnectionManager::new(url))
            .unwrap();
        let source = diesel::sql_query(
            "SELECT id,user_id FROM landscape_analyses ORDER BY created_at,id LIMIT 1",
        )
        .get_result::<AnalysisRow>(&mut pool.get().unwrap())
        .unwrap();
        let existing = AnalysisSummary::find_for_analysis(source.id, &pool).unwrap();
        let legacy = existing
            .iter()
            .find(|s| s.title == "[emoji test] Legacy recap")
            .unwrap();
        assert!(
            legacy.activity_emoji.is_none()
                && legacy.mood_emoji.is_none()
                && legacy.emoji.is_none()
        );

        // One recap per analysis: exercise creation on another empty analysis.
        let source = diesel::sql_query("SELECT a.id,a.user_id FROM landscape_analyses a WHERE NOT EXISTS (SELECT 1 FROM analysis_summaries s WHERE s.landscape_analysis_id=a.id) ORDER BY a.created_at,a.id LIMIT 1")
            .get_result::<AnalysisRow>(&mut pool.get().unwrap()).unwrap();

        let payload: NewAnalysisSummaryDto = serde_json::from_value(serde_json::json!({
            "title":"Emoji persistence test", "content":"Recap", "activity_emoji":"🌱",
            "mood_emoji":"😌", "dominant_emoji_kind":"mood"
        }))
        .unwrap();
        let mut summary = NewAnalysisSummary::new(payload, source.id, source.user_id)
            .create(&pool)
            .unwrap();
        assert_eq!(summary.emoji.as_deref(), Some("😌"));
        assert_eq!(
            AnalysisSummary::find(summary.id, &pool)
                .unwrap()
                .dominant_emoji_kind,
            Some(SummaryEmojiKind::Mood)
        );
        summary.activity_emoji = Some("👩🏽‍🔬".into());
        summary.mood_emoji = None;
        summary = summary.update(&pool).unwrap();
        assert_eq!(summary.emoji.as_deref(), Some("👩🏽‍🔬"));
        assert_eq!(
            summary.dominant_emoji_kind,
            Some(SummaryEmojiKind::Activity)
        );
        let stored = diesel::sql_query(
            "SELECT dominant_emoji_kind AS kind FROM analysis_summaries WHERE id=$1",
        )
        .bind::<diesel::sql_types::Uuid, _>(summary.id)
        .get_result::<StoredKind>(&mut pool.get().unwrap())
        .unwrap();
        assert_eq!(stored.kind, "ACTIVITY");
        summary.activity_emoji = None;
        summary = summary.update(&pool).unwrap();
        assert!(summary.emoji.is_none() && summary.dominant_emoji_kind.is_none());
        diesel::delete(analysis_summaries::table.filter(analysis_summaries::id.eq(summary.id)))
            .execute(&mut pool.get().unwrap())
            .unwrap();
    }
}
