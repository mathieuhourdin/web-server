use axum::{
    debug_handler,
    extract::{Extension, Path, Query},
    Json,
};
use chrono::{Duration, NaiveDate, Utc};
use diesel::{
    prelude::*,
    sql_types::{Date, Nullable, Text, Uuid as SqlUuid},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::{SummaryEmojiKind, SummaryEmojis};
use crate::{
    db::DbPool,
    entities_v2::{
        error::{ErrorType, PpdcError},
        lens::Lens,
        session::Session,
        user::User,
    },
};

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmojiHeatmapMode {
    #[default]
    Main,
    Activity,
    Mood,
}

#[derive(Debug, Default, Deserialize)]
pub struct EmojiHeatmapParams {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub days: Option<i64>,
    #[serde(default)]
    pub mode: EmojiHeatmapMode,
    pub lens_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct EmojiHeatmapDay {
    pub day: NaiveDate,
    pub emoji: Option<String>,
    pub activity_emoji: Option<String>,
    pub mood_emoji: Option<String>,
    pub main_emoji: Option<String>,
    pub summary_id: Option<Uuid>,
}

#[derive(QueryableByName)]
struct DailyEmojiRow {
    #[diesel(sql_type=Date)]
    day: NaiveDate,
    #[diesel(sql_type=SqlUuid)]
    summary_id: Uuid,
    #[diesel(sql_type=Nullable<Text>)]
    activity_emoji: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    mood_emoji: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    dominant_emoji_kind: Option<String>,
}

fn validate_range(from: NaiveDate, to: NaiveDate) -> Result<(), PpdcError> {
    if from > to || (to - from).num_days() > 3660 {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "`from` must be <= `to` and the range must not exceed 3661 days".into(),
        ));
    }
    Ok(())
}

fn project_day(
    day: NaiveDate,
    row: Option<DailyEmojiRow>,
    mode: EmojiHeatmapMode,
) -> EmojiHeatmapDay {
    let (summary_id, cues) = match row {
        Some(row) => (
            Some(row.summary_id),
            SummaryEmojis {
                activity_emoji: row.activity_emoji,
                mood_emoji: row.mood_emoji,
                dominant_emoji_kind: row
                    .dominant_emoji_kind
                    .as_deref()
                    .and_then(SummaryEmojiKind::from_db),
            }
            .normalized(),
        ),
        None => (None, SummaryEmojis::default()),
    };
    let main_emoji = cues.display_emoji();
    let emoji = match mode {
        EmojiHeatmapMode::Main => main_emoji.clone(),
        EmojiHeatmapMode::Activity => cues.activity_emoji.clone(),
        EmojiHeatmapMode::Mood => cues.mood_emoji.clone(),
    };
    EmojiHeatmapDay {
        day,
        emoji,
        main_emoji,
        activity_emoji: cues.activity_emoji,
        mood_emoji: cues.mood_emoji,
        summary_id,
    }
}

#[debug_handler]
pub async fn get_user_emoji_heatmap_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(user_id): Path<Uuid>,
    Query(params): Query<EmojiHeatmapParams>,
) -> Result<Json<Vec<EmojiHeatmapDay>>, PpdcError> {
    if session.user_id != Some(user_id) {
        return Err(PpdcError::unauthorized());
    }
    let user = User::find(&user_id, &pool)?;
    let timezone = user
        .timezone
        .parse::<chrono_tz::Tz>()
        .unwrap_or(chrono_tz::UTC);
    let to = params
        .to
        .unwrap_or_else(|| Utc::now().with_timezone(&timezone).date_naive());
    let days = params.days.unwrap_or(271);
    if !(1..=3661).contains(&days) || (params.from.is_some() && params.days.is_some()) {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "`days` must be between 1 and 3661 and cannot be combined with `from`".into(),
        ));
    }
    let from = match params.from {
        Some(from) => from,
        None => to
            .checked_sub_signed(Duration::days(days - 1))
            .ok_or_else(|| {
                PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "Date range is out of bounds".into(),
                )
            })?,
    };
    validate_range(from, to)?;
    let lens_id = params.lens_id.or(user.current_lens_id);
    if let Some(id) = lens_id {
        if Lens::find_full_lens(id, &pool)?.user_id != Some(user_id) {
            return Err(PpdcError::unauthorized());
        }
    }
    // Daily recap period_end is exclusive local midnight. Using its final
    // microsecond avoids shifting summaries to tomorrow or using generation day.
    // The selected lens scope excludes other experiments/branches.
    let mut conn = pool.get()?;
    let rows = find_daily_emojis(&mut conn, user_id, lens_id, timezone.name(), from, to)?;
    let mut by_day = rows
        .into_iter()
        .map(|row| (row.day, row))
        .collect::<HashMap<_, _>>();
    let days = (0..=(to - from).num_days())
        .map(|offset| {
            let day = from + Duration::days(offset);
            project_day(day, by_day.remove(&day), params.mode)
        })
        .collect();
    Ok(Json(days))
}

fn find_daily_emojis(
    conn: &mut PgConnection,
    user_id: Uuid,
    lens_id: Option<Uuid>,
    timezone: &str,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<DailyEmojiRow>, PpdcError> {
    Ok(diesel::sql_query("WITH candidates AS (
        SELECT ((a.period_end-INTERVAL '1 microsecond') AT TIME ZONE 'UTC' AT TIME ZONE $4)::date AS day,
            s.id AS summary_id,s.activity_emoji,s.mood_emoji,s.dominant_emoji_kind,
            a.created_at AS analysis_created_at,s.updated_at AS summary_updated_at,a.id AS analysis_id
        FROM analysis_summaries s JOIN landscape_analyses a ON a.id=s.landscape_analysis_id
        WHERE s.user_id=$1 AND a.user_id=$1 AND s.summary_type='PERIOD_RECAP'
            AND a.landscape_analysis_type='DAILY_RECAP' AND a.processing_state='COMPLETED'
            AND EXISTS (SELECT 1 FROM lens_analysis_scopes scope WHERE scope.landscape_analysis_id=a.id AND scope.lens_id=$5)
    ) SELECT DISTINCT ON(day) day,summary_id,activity_emoji,mood_emoji,dominant_emoji_kind
      FROM candidates WHERE day BETWEEN $2 AND $3
      ORDER BY day,analysis_created_at DESC,analysis_id DESC,summary_updated_at DESC,summary_id DESC")
        .bind::<SqlUuid,_>(user_id).bind::<Date,_>(from).bind::<Date,_>(to)
        .bind::<Text,_>(timezone).bind::<Nullable<SqlUuid>,_>(lens_id)
        .load::<DailyEmojiRow>(conn)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modes_do_not_invent_missing_cues_and_ranges_are_bounded() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 10).unwrap();
        let row = || DailyEmojiRow {
            day,
            summary_id: Uuid::new_v4(),
            activity_emoji: Some("🌱".into()),
            mood_emoji: None,
            dominant_emoji_kind: Some("ACTIVITY".into()),
        };
        assert_eq!(
            project_day(day, Some(row()), EmojiHeatmapMode::Main)
                .emoji
                .as_deref(),
            Some("🌱")
        );
        assert_eq!(
            project_day(day, Some(row()), EmojiHeatmapMode::Mood).emoji,
            None
        );
        assert_eq!(
            project_day(day, None, EmojiHeatmapMode::Activity).summary_id,
            None
        );
        assert!(validate_range(day, day).is_ok());
        assert!(validate_range(day, day - Duration::days(1)).is_err());
        assert!(validate_range(day, day + Duration::days(3661)).is_err());
    }

    #[test]
    #[ignore = "Requires an explicitly configured disposable emoji_heatmap_test database"]
    fn daily_emojis_respect_local_days_lens_scope_and_completed_recaps() {
        use diesel::connection::SimpleConnection;
        let url = std::env::var("EMOJI_HEATMAP_TEST_DATABASE_URL")
            .expect("Set disposable emoji heatmap test URL");
        assert_eq!(url::Url::parse(&url).unwrap().path(), "/emoji_heatmap_test");
        let mut conn = PgConnection::establish(&url).unwrap();
        conn.begin_test_transaction().unwrap();
        let owner = Uuid::new_v4();
        let lens = Uuid::new_v4();
        let other = Uuid::new_v4();
        conn.batch_execute(&format!("INSERT INTO users(id,email,first_name,last_name,handle,password,updated_at)
            VALUES('{owner}','{owner}@test.invalid','Emoji','Test','@{owner}','unused',NOW());
            INSERT INTO lenses(id,user_id,processing_state,autoplay) VALUES('{lens}','{owner}','IN_SYNC',FALSE),('{other}','{owner}','IN_SYNC',FALSE);")).unwrap();
        let mut chosen = Uuid::nil();
        for (index, (target, kind, state, start_day, end_day)) in [
            (lens, "DAILY_RECAP", "COMPLETED", 10, 11),
            (lens, "DAILY_RECAP", "FAILED", 9, 10),
            (lens, "WEEKLY_RECAP", "COMPLETED", 7, 11),
            (other, "DAILY_RECAP", "COMPLETED", 10, 11),
        ]
        .into_iter()
        .enumerate()
        {
            let analysis = Uuid::new_v4();
            let summary = Uuid::new_v4();
            if index == 0 {
                chosen = summary;
            }
            conn.batch_execute(&format!("INSERT INTO landscape_analyses(id,user_id,title,subtitle,plain_text_state_summary,
                interaction_date,period_start,period_end,landscape_analysis_type,processing_state,created_at)
                VALUES('{analysis}','{owner}','','','','2026-10-10 12:00','2026-10-{start_day:02} 07:00','2026-10-{end_day:02} 07:00','{kind}','{state}','2026-10-{} 12:00');
                INSERT INTO lens_analysis_scopes(id,lens_id,landscape_analysis_id) VALUES(uuid_generate_v4(),'{target}','{analysis}');
                INSERT INTO analysis_summaries(id,landscape_analysis_id,user_id,summary_type,title,short_content,content,activity_emoji,mood_emoji,dominant_emoji_kind)
                VALUES('{summary}','{analysis}','{owner}','PERIOD_RECAP','','','','🌱','😌','MOOD');",index+11)).unwrap();
        }
        let day = NaiveDate::from_ymd_opt(2026, 10, 10).unwrap();
        let mut rows = find_daily_emojis(
            &mut conn,
            owner,
            Some(lens),
            "America/Los_Angeles",
            day - Duration::days(1),
            day,
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].day, day);
        assert_eq!(rows[0].summary_id, chosen);
        assert_eq!(
            project_day(day, Some(rows.remove(0)), EmojiHeatmapMode::Main)
                .emoji
                .as_deref(),
            Some("😌")
        );
        assert!(
            find_daily_emojis(&mut conn, owner, None, "America/Los_Angeles", day, day)
                .unwrap()
                .is_empty()
        );
        assert!(find_daily_emojis(
            &mut conn,
            Uuid::new_v4(),
            Some(lens),
            "America/Los_Angeles",
            day,
            day
        )
        .unwrap()
        .is_empty());
        assert!(
            find_daily_emojis(&mut conn, owner, Some(lens), "UTC", day, day)
                .unwrap()
                .is_empty(),
            "The UTC date is tomorrow, not this user's summary day"
        );
        // The test transaction rolls back all fixture data on connection drop.
    }
}
