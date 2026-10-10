use chrono::NaiveDateTime;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::schema::analysis_summaries;

use super::model::{AnalysisSummary, AnalysisSummaryType, SummaryEmojiKind, SummaryEmojis};

type AnalysisSummaryTuple = (
    Uuid,
    Uuid,
    Uuid,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    NaiveDateTime,
    NaiveDateTime,
    Option<String>,
    Option<String>,
    Option<String>,
);

fn tuple_to_analysis_summary(row: AnalysisSummaryTuple) -> AnalysisSummary {
    let (
        id,
        landscape_analysis_id,
        user_id,
        summary_type_raw,
        title,
        short_content,
        content,
        meaningful_event_title,
        meaningful_event_description,
        meaningful_event_date,
        created_at,
        updated_at,
        activity_emoji,
        mood_emoji,
        dominant_emoji_kind,
    ) = row;

    let emojis = SummaryEmojis {
        activity_emoji,
        mood_emoji,
        dominant_emoji_kind: dominant_emoji_kind
            .as_deref()
            .and_then(SummaryEmojiKind::from_db),
    }
    .normalized();
    let emoji = emojis.display_emoji();

    AnalysisSummary {
        id,
        landscape_analysis_id,
        user_id,
        summary_type: AnalysisSummaryType::from_db(&summary_type_raw),
        title,
        short_content,
        content,
        meaningful_event: match (
            meaningful_event_title,
            meaningful_event_description,
            meaningful_event_date,
        ) {
            (Some(title), Some(description), Some(event_date)) => {
                Some(super::model::MeaningfulEvent {
                    title,
                    description,
                    event_date,
                })
            }
            _ => None,
        },
        created_at,
        updated_at,
        activity_emoji: emojis.activity_emoji,
        mood_emoji: emojis.mood_emoji,
        dominant_emoji_kind: emojis.dominant_emoji_kind,
        emoji,
    }
}

fn select_analysis_summary_columns() -> (
    analysis_summaries::id,
    analysis_summaries::landscape_analysis_id,
    analysis_summaries::user_id,
    analysis_summaries::summary_type,
    analysis_summaries::title,
    analysis_summaries::short_content,
    analysis_summaries::content,
    analysis_summaries::meaningful_event_title,
    analysis_summaries::meaningful_event_description,
    analysis_summaries::meaningful_event_date,
    analysis_summaries::created_at,
    analysis_summaries::updated_at,
    analysis_summaries::activity_emoji,
    analysis_summaries::mood_emoji,
    analysis_summaries::dominant_emoji_kind,
) {
    (
        analysis_summaries::id,
        analysis_summaries::landscape_analysis_id,
        analysis_summaries::user_id,
        analysis_summaries::summary_type,
        analysis_summaries::title,
        analysis_summaries::short_content,
        analysis_summaries::content,
        analysis_summaries::meaningful_event_title,
        analysis_summaries::meaningful_event_description,
        analysis_summaries::meaningful_event_date,
        analysis_summaries::created_at,
        analysis_summaries::updated_at,
        analysis_summaries::activity_emoji,
        analysis_summaries::mood_emoji,
        analysis_summaries::dominant_emoji_kind,
    )
}

impl AnalysisSummary {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<AnalysisSummary, PpdcError> {
        let mut conn = pool.get()?;
        let row = analysis_summaries::table
            .filter(analysis_summaries::id.eq(id))
            .select(select_analysis_summary_columns())
            .first::<AnalysisSummaryTuple>(&mut conn)
            .optional()?;
        row.map(tuple_to_analysis_summary).ok_or_else(|| {
            PpdcError::new(
                404,
                ErrorType::ApiError,
                "Analysis summary not found".to_string(),
            )
        })
    }

    pub fn find_for_analysis(
        landscape_analysis_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<AnalysisSummary>, PpdcError> {
        let mut conn = pool.get()?;
        let rows = analysis_summaries::table
            .filter(analysis_summaries::landscape_analysis_id.eq(landscape_analysis_id))
            .select(select_analysis_summary_columns())
            .order(analysis_summaries::created_at.desc())
            .load::<AnalysisSummaryTuple>(&mut conn)?;
        Ok(rows.into_iter().map(tuple_to_analysis_summary).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> AnalysisSummaryTuple {
        let now = chrono::Utc::now().naive_utc();
        (
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "PERIOD_RECAP".into(),
            "Day".into(),
            "Short".into(),
            "Full recap".into(),
            None,
            None,
            None,
            now,
            now,
            None,
            None,
            None,
        )
    }

    #[test]
    fn legacy_summary_returns_explicit_null_emoji_fields() {
        let summary = tuple_to_analysis_summary(row());
        let json = serde_json::to_value(summary).unwrap();
        for field in [
            "activity_emoji",
            "mood_emoji",
            "dominant_emoji_kind",
            "emoji",
        ] {
            assert_eq!(json.get(field), Some(&serde_json::Value::Null));
        }
    }

    #[test]
    fn read_api_projects_the_selected_display_emoji() {
        let mut data = row();
        data.12 = Some("🌱".into());
        data.13 = Some("😌".into());
        data.14 = Some("MOOD".into());
        let json = serde_json::to_value(tuple_to_analysis_summary(data)).unwrap();
        assert_eq!(json["activity_emoji"], "🌱");
        assert_eq!(json["mood_emoji"], "😌");
        assert_eq!(json["dominant_emoji_kind"], "mood");
        assert_eq!(json["emoji"], "😌");
    }
}
