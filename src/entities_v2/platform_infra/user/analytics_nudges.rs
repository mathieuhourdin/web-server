//! Read-time nudge analytics over accepted, deduplicated usage events.
use super::admin::{
    ensure_admin_session_user, resolve_admin_analytics_date_range, AdminAnalyticsDateRangeQuery,
    AdminAnalyticsPoint, AdminPlatformAnalyticsMetric,
};
use crate::{
    db::DbPool,
    entities_v2::platform_infra::timezone::postgres_timezone_sql,
    entities_v2::{
        error::{ErrorType, PpdcError},
        session::Session,
    },
};
use axum::{
    extract::{Extension, Query},
    Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Date, Text, Timestamp},
};
use serde::Serialize;

pub(super) fn metric_event(metric: AdminPlatformAnalyticsMetric) -> Option<(&'static str, bool)> {
    use AdminPlatformAnalyticsMetric::*;
    Some(match metric {
        WritingNudgeShown => ("WRITING_NUDGE_SHOWN", false),
        WritingNudgeOpened => ("WRITING_NUDGE_OPENED", false),
        WritingNudgeDismissed => ("WRITING_NUDGE_DISMISSED", false),
        WritingNudgeShownUsers => ("WRITING_NUDGE_SHOWN", true),
        WritingNudgeOpenedUsers => ("WRITING_NUDGE_OPENED", true),
        WritingNudgeDismissedUsers => ("WRITING_NUDGE_DISMISSED", true),
        _ => return None,
    })
}

#[derive(QueryableByName)]
struct SeriesRow {
    #[diesel(sql_type=Date)]
    day: NaiveDate,
    #[diesel(sql_type=BigInt)]
    count: i64,
}

pub(super) fn load_series(
    event_type: &str,
    distinct_users: bool,
    from: NaiveDate,
    to: NaiveDate,
    conn: &mut PgConnection,
) -> Result<Vec<AdminAnalyticsPoint>, PpdcError> {
    let rows = diesel::sql_query(format!("WITH days AS (
        SELECT generate_series($1::date::timestamp,$2::date::timestamp,interval '1 day')::date AS day
    ), activity AS (
        SELECT timezone({timezone},ue.occurred_at AT TIME ZONE 'UTC')::date AS day,
            {count}::bigint AS count
        FROM usage_events ue JOIN analytics_users u ON u.id=ue.user_id
        WHERE u.principal_type='HUMAN' AND u.is_platform_user=TRUE AND ue.event_type=$3
          AND ue.occurred_at>=($1::date-interval '1 day')
          AND ue.occurred_at<($2::date+interval '2 days')
          AND ue.occurred_at<=$4
        GROUP BY 1
    ) SELECT d.day,COALESCE(a.count,0)::bigint AS count
      FROM days d LEFT JOIN activity a ON a.day=d.day ORDER BY d.day",
        timezone=postgres_timezone_sql("u.timezone"),
        count=if distinct_users {"COUNT(DISTINCT ue.user_id)"} else {"COUNT(*)"},
    ))
    .bind::<Date,_>(from).bind::<Date,_>(to).bind::<Text,_>(event_type)
    .bind::<Timestamp,_>(Utc::now().naive_utc()).load::<SeriesRow>(conn)?;
    Ok(rows
        .into_iter()
        .map(|r| AdminAnalyticsPoint {
            date: r.day,
            count: r.count,
            total_users: None,
            percentage: None,
        })
        .collect())
}

#[derive(QueryableByName)]
struct FunnelRow {
    #[diesel(sql_type=Text)]
    nudge_type: String,
    #[diesel(sql_type=BigInt)]
    shown: i64,
    #[diesel(sql_type=BigInt)]
    opened: i64,
    #[diesel(sql_type=BigInt)]
    dismissed: i64,
    #[diesel(sql_type=BigInt)]
    shown_users: i64,
    #[diesel(sql_type=BigInt)]
    opened_users: i64,
    #[diesel(sql_type=BigInt)]
    dismissed_users: i64,
}

#[derive(Serialize)]
pub struct WritingNudgeFunnel {
    nudge_type: String,
    shown: i64,
    opened: i64,
    dismissed: i64,
    no_recorded_action: i64,
    shown_users: i64,
    opened_users: i64,
    dismissed_users: i64,
    opening_rate: f64,
    dismissal_rate: f64,
}

impl From<FunnelRow> for WritingNudgeFunnel {
    fn from(row: FunnelRow) -> Self {
        let percent = |count| {
            if row.shown == 0 {
                0.0
            } else {
                100.0 * count as f64 / row.shown as f64
            }
        };
        Self {
            opening_rate: percent(row.opened),
            dismissal_rate: percent(row.dismissed),
            no_recorded_action: row.shown - row.opened - row.dismissed,
            nudge_type: row.nudge_type,
            shown: row.shown,
            opened: row.opened,
            dismissed: row.dismissed,
            shown_users: row.shown_users,
            opened_users: row.opened_users,
            dismissed_users: row.dismissed_users,
        }
    }
}

#[derive(Serialize)]
pub struct WritingNudgeAnalytics {
    from: NaiveDate,
    to: NaiveDate,
    as_of: DateTime<Utc>,
    types: Vec<WritingNudgeFunnel>,
}

fn load_funnel(
    from: NaiveDate,
    to: NaiveDate,
    as_of: DateTime<Utc>,
    conn: &mut PgConnection,
) -> Result<Vec<WritingNudgeFunnel>, PpdcError> {
    // Cohort by exposure date, not terminal-event date. Immutable usage-event
    // context survives replacement of a user's current nudge state.
    let rows = diesel::sql_query(format!("WITH cohort AS (
        SELECT ue.user_id,ue.resource_id,ue.context_json->>'nudge_type' AS nudge_type,ue.occurred_at
        FROM usage_events ue JOIN analytics_users u ON u.id=ue.user_id
        WHERE u.principal_type='HUMAN' AND u.is_platform_user=TRUE
          AND ue.event_type='WRITING_NUDGE_SHOWN' AND ue.resource_id IS NOT NULL
          AND ue.occurred_at>=($1::date-interval '1 day')
          AND ue.occurred_at<($2::date+interval '2 days') AND ue.occurred_at<=$3
          AND timezone({timezone},ue.occurred_at AT TIME ZONE 'UTC')::date BETWEEN $1 AND $2
    ), outcomes AS (
        SELECT c.user_id,c.resource_id,c.nudge_type,
            COALESCE(BOOL_OR(e.event_type='WRITING_NUDGE_OPENED'),FALSE) AS opened,
            COALESCE(BOOL_OR(e.event_type='WRITING_NUDGE_DISMISSED'),FALSE) AS dismissed
        FROM cohort c LEFT JOIN usage_events e ON e.user_id=c.user_id AND e.resource_id=c.resource_id
          AND e.event_type IN ('WRITING_NUDGE_OPENED','WRITING_NUDGE_DISMISSED')
          AND e.occurred_at>=c.occurred_at AND e.occurred_at<=$3
        GROUP BY c.user_id,c.resource_id,c.nudge_type
    ), kinds(nudge_type) AS (VALUES ('resume_draft'),('writing_memory'),('first_trace'))
    SELECT k.nudge_type,COUNT(o.resource_id)::bigint AS shown,
        COUNT(*) FILTER (WHERE o.opened)::bigint AS opened,
        COUNT(*) FILTER (WHERE o.dismissed)::bigint AS dismissed,
        COUNT(DISTINCT o.user_id)::bigint AS shown_users,
        COUNT(DISTINCT o.user_id) FILTER (WHERE o.opened)::bigint AS opened_users,
        COUNT(DISTINCT o.user_id) FILTER (WHERE o.dismissed)::bigint AS dismissed_users
    FROM kinds k LEFT JOIN outcomes o ON o.nudge_type=k.nudge_type
    GROUP BY k.nudge_type ORDER BY k.nudge_type", timezone=postgres_timezone_sql("u.timezone")))
    .bind::<Date,_>(from).bind::<Date,_>(to).bind::<Timestamp,_>(as_of.naive_utc())
    .load::<FunnelRow>(conn)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_admin_writing_nudges_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<AdminAnalyticsDateRangeQuery>,
) -> Result<Json<WritingNudgeAnalytics>, PpdcError> {
    let response = tokio::task::spawn_blocking(move || {
        ensure_admin_session_user(&session, &pool)?;
        let as_of = Utc::now();
        let (from, to) =
            resolve_admin_analytics_date_range(params.from, params.to, as_of.date_naive())?;
        let mut conn = pool.get()?;
        let types = load_funnel(from, to, as_of, &mut conn)?;
        Ok::<_, PpdcError>(WritingNudgeAnalytics {
            from,
            to,
            as_of,
            types,
        })
    })
    .await
    .map_err(|_| {
        PpdcError::new(
            500,
            ErrorType::InternalError,
            "Nudge analytics lookup failed".into(),
        )
    })??;
    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities_v2::usage_event::UsageEventType;
    use diesel::connection::SimpleConnection;

    #[test]
    fn nudge_metric_names_match_events_and_zero_rates_are_finite() {
        for name in [
            "writing_nudge_shown",
            "writing_nudge_opened",
            "writing_nudge_dismissed",
        ] {
            let event: UsageEventType = serde_json::from_value(serde_json::json!(name)).unwrap();
            assert_eq!(UsageEventType::from_db(event.to_db()).unwrap(), event);
            assert!(include_str!(
                "../../../../migrations/2026-11-04-110000_writing_nudge_usage_events/up.sql"
            )
            .contains(event.to_db()));
            for (metric_name, distinct) in
                [(name.to_string(), false), (format!("{name}_users"), true)]
            {
                let metric = serde_json::from_value(serde_json::json!(metric_name)).unwrap();
                assert_eq!(metric_event(metric), Some((event.to_db(), distinct)));
            }
        }
        let result = WritingNudgeFunnel::from(FunnelRow {
            nudge_type: "first_trace".into(),
            shown: 0,
            opened: 0,
            dismissed: 0,
            shown_users: 0,
            opened_users: 0,
            dismissed_users: 0,
        });
        assert_eq!(result.opening_rate, 0.0);
        assert_eq!(result.no_recorded_action, 0);
    }

    #[test]
    #[ignore = "Requires explicitly configured disposable blog_test; metadata-only temporary tables"]
    fn exposure_cohorts_daily_counts_and_distinct_users() {
        let url = std::env::var("BLOG_TEST_DATABASE_URL").unwrap();
        assert_eq!(url::Url::parse(&url).unwrap().path(), "/blog_test");
        let mut conn = PgConnection::establish(&url).unwrap();
        // Shadow real tables only for this connection. No source content needed.
        conn.batch_execute("CREATE TEMP TABLE users(id uuid,principal_type text,is_platform_user boolean,timezone text);
            CREATE TEMP TABLE usage_events(user_id uuid,resource_id uuid,event_type text,context_json jsonb,occurred_at timestamp);
            INSERT INTO users VALUES
                ('00000000-0000-0000-0000-000000000001','HUMAN',TRUE,'Europe/Paris'),
                ('00000000-0000-0000-0000-000000000002','HUMAN',TRUE,'invalid/zone'),
                ('00000000-0000-0000-0000-000000000003','SERVICE',TRUE,'UTC');
            INSERT INTO usage_events VALUES
                ('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000011','WRITING_NUDGE_SHOWN','{\"nudge_type\":\"first_trace\"}','2026-09-30 23:00:00'),
                ('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000011','WRITING_NUDGE_OPENED','{}','2026-10-02 01:00:00'),
                ('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000012','WRITING_NUDGE_SHOWN','{\"nudge_type\":\"first_trace\"}','2026-10-01 10:00:00'),
                ('00000000-0000-0000-0000-000000000002','00000000-0000-0000-0000-000000000013','WRITING_NUDGE_SHOWN','{\"nudge_type\":\"first_trace\"}','2026-10-01 10:00:00'),
                ('00000000-0000-0000-0000-000000000002','00000000-0000-0000-0000-000000000013','WRITING_NUDGE_DISMISSED','{}','2026-10-01 10:00:00'),
                ('00000000-0000-0000-0000-000000000003','00000000-0000-0000-0000-000000000014','WRITING_NUDGE_SHOWN','{\"nudge_type\":\"first_trace\"}','2026-10-01 10:00:00');").unwrap();
        let day = "2026-10-01".parse().unwrap();
        let before =
            load_funnel(day, day, "2026-10-01T23:59:00Z".parse().unwrap(), &mut conn).unwrap();
        let first = before
            .iter()
            .find(|r| r.nudge_type == "first_trace")
            .unwrap();
        assert_eq!(
            (
                first.shown,
                first.opened,
                first.dismissed,
                first.no_recorded_action
            ),
            (3, 0, 1, 2)
        );
        let after =
            load_funnel(day, day, "2026-10-03T00:00:00Z".parse().unwrap(), &mut conn).unwrap();
        let first = after
            .iter()
            .find(|r| r.nudge_type == "first_trace")
            .unwrap();
        assert_eq!(
            (
                first.shown,
                first.shown_users,
                first.opened,
                first.dismissed,
                first.no_recorded_action
            ),
            (3, 2, 1, 1, 1)
        );
        assert!((first.opening_rate - 100.0 / 3.0).abs() < 1e-10);
        assert_eq!(after.len(), 3);
        assert!(after
            .iter()
            .filter(|r| r.nudge_type != "first_trace")
            .all(|r| r.shown == 0 && r.opening_rate == 0.0));
        for (distinct, expected) in [(false, 3), (true, 2)] {
            let rows = load_series("WRITING_NUDGE_SHOWN", distinct, day, day, &mut conn).unwrap();
            assert_eq!(rows[0].count, expected);
        }
        let rows = load_series("WRITING_NUDGE_OPENED", false, day, day, &mut conn).unwrap();
        assert_eq!(rows[0].count, 0); // opened later counts in cohort, not daily Oct 1 series.
    }
}
