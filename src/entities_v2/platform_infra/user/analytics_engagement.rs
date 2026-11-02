//! Read-time engagement metrics; no persisted aggregates or per-user queries.
use super::admin::{ensure_admin_session_user, AdminAnalyticsPoint, AdminPlatformAnalyticsMetric};
use crate::{
    db::DbPool,
    entities_v2::{error::PpdcError, session::Session},
};
use axum::{extract::Extension, Json};
use chrono::{DateTime, NaiveDate, Utc};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Date, Integer, Timestamp},
};
use serde::Serialize;

const USERS: &str = "eligible_users AS (
    SELECT id,created_at FROM users WHERE principal_type='HUMAN' AND is_platform_user=TRUE
)";
const READ_EVENTS: &str =
    "'FOLLOWED_JOURNAL_OPENED','POST_OPENED','DOCUMENT_OPENED','ALBUM_OPENED'";

fn classified_traces(cutoff: &str) -> String {
    format!(
        "classified_traces AS (
        SELECT t.id,t.user_id,t.finalized_at,
            (t.status='FINALIZED' AND ({shared})) AS shared
        FROM traces t JOIN eligible_users u ON u.id=t.user_id
        WHERE t.finalized_at IS NOT NULL AND t.finalized_at <= {cutoff}
          AND t.trace_type IN ('USER_TRACE','WORKSPACE_TRACE')
    )",
        shared = crate::entities_v2::social::feed::sharing::TRACE_IS_SHARED_SQL
    )
}

fn writing_series_sql(publishers: bool) -> String {
    let traces = classified_traces("$3");
    if publishers {
        // First publication, not last_distribution_at: redistributing a trace
        // must not move its original publishing activity to a different day.
        format!("WITH {USERS}, {traces},
            first_events AS (
                SELECT post_id,MIN(distributed_at) AS first_distributed_at
                FROM post_distribution_events GROUP BY post_id
            ),
            publications AS (
                SELECT t.id,t.user_id,MIN(COALESCE(e.first_distributed_at,p.publishing_date,p.created_at)) AS published_at
                FROM classified_traces t JOIN posts p ON p.source_trace_id=t.id AND p.user_id=t.user_id
                LEFT JOIN first_events e ON e.post_id=p.id
                WHERE t.shared AND p.status='PUBLISHED'
                GROUP BY t.id,t.user_id
            ),
            days AS (SELECT generate_series($1::date::timestamp,$2::date::timestamp,interval '1 day')::date AS day)
            SELECT d.day,COUNT(DISTINCT p.user_id)::bigint AS count
            FROM days d LEFT JOIN publications p ON p.published_at>=d.day::timestamp
                AND p.published_at<d.day+interval '1 day' AND p.published_at<=$3
            GROUP BY d.day ORDER BY d.day")
    } else {
        format!(
            "WITH {USERS}, {traces},
            days AS (SELECT generate_series($1::date::timestamp,$2::date::timestamp,interval '1 day')::date AS day)
            SELECT d.day,COUNT(DISTINCT t.user_id)::bigint AS count
            FROM days d LEFT JOIN classified_traces t ON NOT t.shared
                AND t.finalized_at>=d.day::timestamp AND t.finalized_at<d.day+interval '1 day'
            GROUP BY d.day ORDER BY d.day"
        )
    }
}

fn inactivity_series_sql() -> String {
    // Ranking and disjoint activity intervals preserve historical qualification:
    // a user's third trace in the future cannot qualify them for an earlier day.
    format!("WITH {USERS}, writing_history AS (
        SELECT t.user_id,t.finalized_at,
            ROW_NUMBER() OVER (PARTITION BY t.user_id ORDER BY t.finalized_at,t.id) AS ordinal,
            LEAD(t.finalized_at) OVER (PARTITION BY t.user_id ORDER BY t.finalized_at,t.id) AS next_written_at
        FROM traces t JOIN eligible_users u ON u.id=t.user_id
        WHERE t.finalized_at IS NOT NULL AND t.finalized_at<=$3
          AND t.trace_type IN ('USER_TRACE','WORKSPACE_TRACE')
    ), days AS (
        SELECT day::date AS day,LEAST(day+interval '1 day',$3::timestamp) AS cutoff
        FROM generate_series($1::date::timestamp,$2::date::timestamp,interval '1 day') AS day
    )
    SELECT d.day,COUNT(h.user_id)::bigint AS count
    FROM days d LEFT JOIN writing_history h ON h.ordinal>=3 AND d.day<=$3::date
        AND h.finalized_at<d.cutoff-make_interval(days=>$4)
        AND (h.next_written_at IS NULL OR h.next_written_at>=d.cutoff)
    GROUP BY d.day ORDER BY d.day")
}

#[derive(QueryableByName)]
struct SeriesRow {
    #[diesel(sql_type=Date)]
    day: NaiveDate,
    #[diesel(sql_type=BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct ShareRow {
    #[diesel(sql_type=Date)]
    day: NaiveDate,
    #[diesel(sql_type=BigInt)]
    count: i64,
    #[diesel(sql_type=BigInt)]
    total_users: i64,
}

impl ShareRow {
    fn into_point(self) -> AdminAnalyticsPoint {
        AdminAnalyticsPoint {
            date: self.day,
            count: self.count,
            total_users: Some(self.total_users),
            percentage: Some(if self.total_users == 0 {
                0.0
            } else {
                100.0 * self.count as f64 / self.total_users as f64
            }),
        }
    }
}

fn share_series_sql(metric: AdminPlatformAnalyticsMetric) -> String {
    let (history, join, condition) = match metric {
        AdminPlatformAnalyticsMetric::PublishersShare | AdminPlatformAnalyticsMetric::PrivateWritersShare => {
            let traces=classified_traces("$3");
            let history=format!("{traces}, activity AS (
                SELECT user_id,finalized_at AS activity_at,
                    LEAD(finalized_at) OVER chronology AS next_activity_at,
                    ROW_NUMBER() OVER chronology AS written,
                    SUM(CASE WHEN shared THEN 1 ELSE 0 END) OVER chronology AS shared,
                    SUM(CASE WHEN NOT shared THEN 1 ELSE 0 END) OVER chronology AS private
                FROM classified_traces
                WINDOW chronology AS (PARTITION BY user_id ORDER BY finalized_at,id
                    ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
            )");
            let condition=if metric==AdminPlatformAnalyticsMetric::PublishersShare {
                "a.shared>=3"
            } else {
                "a.written>=3 AND a.private*2>a.written"
            };
            (history, "a.activity_at<d.cutoff", condition)
        }
        AdminPlatformAnalyticsMetric::ReadersShare => (
            format!("activity AS (
                SELECT ue.user_id,ue.occurred_at AS activity_at,
                    LEAD(ue.occurred_at) OVER (PARTITION BY ue.user_id ORDER BY ue.occurred_at,ue.id) AS next_activity_at
                FROM usage_events ue JOIN eligible_users u ON u.id=ue.user_id
                WHERE ue.event_type IN ({READ_EVENTS}) AND ue.occurred_at<=$3
                  AND ue.occurred_at>=$1::date-interval '30 days'
            )"),
            "a.activity_at<d.cutoff AND a.activity_at>=d.cutoff-interval '30 days'",
            "a.user_id IS NOT NULL",
        ),
        _ => unreachable!("Only share metrics use this query"),
    };
    // At most one disjoint activity interval matches each account/day. Count all
    // accounts, not only active users, and never apply the exclusive snapshot's
    // churn-first priority or its reader <3-writing restriction.
    format!(
        "WITH {USERS}, {history}, days AS (
        SELECT day::date AS day,LEAST(day+interval '1 day',$3::timestamp) AS cutoff
        FROM generate_series($1::date::timestamp,$2::date::timestamp,interval '1 day') AS day
    )
    SELECT d.day,COUNT(u.id)::bigint AS total_users,
        COUNT(u.id) FILTER(WHERE {condition})::bigint AS count
    FROM days d LEFT JOIN eligible_users u ON u.created_at<d.cutoff AND d.day<=$3::date
    LEFT JOIN activity a ON a.user_id=u.id AND ({join})
        AND (a.next_activity_at IS NULL OR a.next_activity_at>=d.cutoff)
    GROUP BY d.day ORDER BY d.day"
    )
}

pub(super) fn load_series(
    metric: AdminPlatformAnalyticsMetric,
    from: NaiveDate,
    to: NaiveDate,
    conn: &mut PgConnection,
) -> Result<Vec<AdminAnalyticsPoint>, PpdcError> {
    let as_of = Utc::now().naive_utc();
    if matches!(
        metric,
        AdminPlatformAnalyticsMetric::PublishersShare
            | AdminPlatformAnalyticsMetric::PrivateWritersShare
            | AdminPlatformAnalyticsMetric::ReadersShare
    ) {
        return Ok(diesel::sql_query(share_series_sql(metric))
            .bind::<Date, _>(from)
            .bind::<Date, _>(to)
            .bind::<Timestamp, _>(as_of)
            .load::<ShareRow>(conn)?
            .into_iter()
            .map(ShareRow::into_point)
            .collect());
    }
    let rows = match metric {
        AdminPlatformAnalyticsMetric::DailyPublishers
        | AdminPlatformAnalyticsMetric::DailyPrivateWriters => diesel::sql_query(
            writing_series_sql(metric == AdminPlatformAnalyticsMetric::DailyPublishers),
        )
        .bind::<Date, _>(from)
        .bind::<Date, _>(to)
        .bind::<Timestamp, _>(as_of)
        .load::<SeriesRow>(conn)?,
        AdminPlatformAnalyticsMetric::InactiveWriters30d
        | AdminPlatformAnalyticsMetric::InactiveWriters60d => {
            let days = if metric == AdminPlatformAnalyticsMetric::InactiveWriters30d {
                30
            } else {
                60
            };
            diesel::sql_query(inactivity_series_sql())
                .bind::<Date, _>(from)
                .bind::<Date, _>(to)
                .bind::<Timestamp, _>(as_of)
                .bind::<Integer, _>(days)
                .load::<SeriesRow>(conn)?
        }
        _ => unreachable!("Only engagement series are dispatched here"),
    };
    Ok(rows
        .into_iter()
        .map(|row| AdminAnalyticsPoint {
            date: row.day,
            count: row.count,
            total_users: None,
            percentage: None,
        })
        .collect())
}

#[derive(QueryableByName, Serialize)]
pub struct UserSegments {
    #[diesel(sql_type=BigInt)]
    pub publishers: i64,
    #[diesel(sql_type=BigInt)]
    pub private_writers: i64,
    #[diesel(sql_type=BigInt)]
    pub readers: i64,
    #[diesel(sql_type=BigInt)]
    pub churners: i64,
    #[diesel(sql_type=BigInt)]
    pub other: i64,
}

#[derive(Serialize)]
pub struct AdminUserSegments {
    pub as_of: DateTime<Utc>,
    pub total_users: i64,
    pub segments: UserSegments,
}

fn user_segments_sql() -> String {
    let traces = classified_traces("$1");
    format!("WITH {USERS}, {traces},
        writing AS (
            SELECT user_id,COUNT(*) AS written,COUNT(*) FILTER(WHERE shared) AS shared,
                COUNT(*) FILTER(WHERE NOT shared) AS private,MAX(finalized_at) AS last_written_at
            FROM classified_traces GROUP BY user_id
        ), reading AS (
            SELECT ue.user_id,MAX(ue.occurred_at) AS last_read_at
            FROM usage_events ue JOIN eligible_users u ON u.id=ue.user_id
            WHERE ue.event_type IN ({READ_EVENTS})
              AND ue.occurred_at>=$1-interval '30 days' AND ue.occurred_at<=$1
            GROUP BY ue.user_id
        ), classified_users AS (
            SELECT CASE
                WHEN u.created_at<$1-interval '30 days'
                    AND COALESCE(w.last_written_at,u.created_at)<$1-interval '30 days'
                    AND COALESCE(r.last_read_at,u.created_at)<$1-interval '30 days' THEN 'churners'
                WHEN w.written>=3 AND w.private*2>w.written THEN 'private_writers'
                WHEN w.shared>=3 THEN 'publishers'
                WHEN COALESCE(w.written,0)<3 AND r.last_read_at>=$1-interval '30 days' THEN 'readers'
                ELSE 'other' END AS segment
            FROM eligible_users u LEFT JOIN writing w ON w.user_id=u.id
            LEFT JOIN reading r ON r.user_id=u.id WHERE u.created_at<=$1
        )
        SELECT COUNT(*) FILTER(WHERE segment='publishers')::bigint AS publishers,
            COUNT(*) FILTER(WHERE segment='private_writers')::bigint AS private_writers,
            COUNT(*) FILTER(WHERE segment='readers')::bigint AS readers,
            COUNT(*) FILTER(WHERE segment='churners')::bigint AS churners,
            COUNT(*) FILTER(WHERE segment='other')::bigint AS other FROM classified_users")
}

pub async fn get_admin_user_segments_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<AdminUserSegments>, PpdcError> {
    ensure_admin_session_user(&session, &pool)?;
    let as_of = Utc::now();
    let segments = diesel::sql_query(user_segments_sql())
        .bind::<Timestamp, _>(as_of.naive_utc())
        .get_result::<UserSegments>(&mut pool.get()?)?;
    let total_users = segments.publishers
        + segments.private_writers
        + segments.readers
        + segments.churners
        + segments.other;
    Ok(Json(AdminUserSegments {
        as_of,
        total_users,
        segments,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities_v2::usage_event::{create_usage_event, UsageEventType};
    use diesel::{connection::SimpleConnection, r2d2::ConnectionManager};
    use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
    use uuid::Uuid;
    const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

    #[test]
    fn new_metric_and_event_names_match_the_contract_and_db_constraint() {
        for name in [
            "daily_publishers",
            "daily_private_writers",
            "inactive_writers_30d",
            "inactive_writers_60d",
            "document_opened",
            "album_opened",
            "publishers_share",
            "private_writers_share",
            "readers_share",
        ] {
            let metric: AdminPlatformAnalyticsMetric =
                serde_json::from_value(serde_json::json!(name)).unwrap();
            assert_eq!(serde_json::to_value(metric).unwrap(), name);
        }
        for name in ["document_opened", "album_opened"] {
            let event: UsageEventType = serde_json::from_value(serde_json::json!(name)).unwrap();
            assert_eq!(UsageEventType::from_db(event.to_db()).unwrap(), event);
            assert!(include_str!(
                "../../../../migrations/2026-11-02-180000_document_album_usage_events/up.sql"
            )
            .contains(event.to_db()));
        }
    }

    #[test]
    fn share_points_preserve_counts_and_return_fractional_percentages() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
        let point = ShareRow {
            day: date,
            count: 2,
            total_users: 7,
        }
        .into_point();
        assert_eq!(point.total_users, Some(7));
        assert!((point.percentage.unwrap() - 200.0 / 7.0).abs() < 1e-10);
        assert_eq!(
            ShareRow {
                day: date,
                count: 0,
                total_users: 0
            }
            .into_point()
            .percentage,
            Some(0.0)
        );
        let ordinary = AdminAnalyticsPoint {
            date,
            count: 2,
            total_users: None,
            percentage: None,
        };
        assert_eq!(
            serde_json::to_value(ordinary).unwrap(),
            serde_json::json!({"date":"2026-11-02","count":2})
        );
    }

    #[test]
    #[ignore = "Requires an empty disposable analytics_test PostgreSQL database"]
    fn engagement_metrics_and_open_events_synthetic_scenario() {
        let url = std::env::var("ANALYTICS_TEST_DATABASE_URL")
            .expect("Set the disposable analytics_test URL");
        assert_eq!(url::Url::parse(&url).unwrap().path(), "/analytics_test");
        let pool = DbPool::builder()
            .max_size(4)
            .build(ConnectionManager::new(url))
            .unwrap();
        let mut conn = pool.get().unwrap();
        conn.run_pending_migrations(MIGRATIONS).unwrap();
        #[derive(QueryableByName)]
        struct Count {
            #[diesel(sql_type=BigInt)]
            value: i64,
        }
        assert_eq!(diesel::sql_query("SELECT COUNT(*)::bigint AS value FROM users WHERE principal_type='HUMAN' AND is_platform_user")
            .get_result::<Count>(&mut conn).unwrap().value,0,"Use a fresh test DB, never user data");
        let now = Utc::now().naive_utc();
        let today = now.date();
        let at = |days| {
            (today - chrono::Duration::days(days))
                .and_hms_opt(12, 0, 0)
                .unwrap()
        };
        let ids: Vec<_> = (0..7).map(|_| Uuid::new_v4()).collect();
        let journals: Vec<_> = (0..7).map(|_| Uuid::new_v4()).collect();
        for (index, user) in ids.iter().enumerate() {
            let created = if index == 4 { now } else { at(150) };
            let journal = journals[index];
            conn.batch_execute(&format!("INSERT INTO users(id,email,first_name,last_name,handle,password,created_at,updated_at,timezone)
                VALUES('{user}','{user}@test.invalid','Synthetic','Test','{user}','unused','{created}',NOW(),'UTC');
                INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status)
                VALUES('{journal}','{user}','Synthetic','','','USER_JOURNAL','ACTIVE');")).unwrap();
        }
        let mut publisher_posts = Vec::new();
        // publisher; private majority also publishing 3; old churner; old active
        // reader; and someone whose third trace did not exist 40 days ago.
        for (user_index, days, shared) in [
            (0, vec![3, 3, 3], 3),
            (1, vec![1; 7], 3),
            (3, vec![100; 3], 0),
            (5, vec![100; 3], 0),
            (6, vec![120, 100, 2], 0),
        ] {
            for (index, days_ago) in days.into_iter().enumerate() {
                let (trace, post) = (Uuid::new_v4(), Uuid::new_v4());
                let user = ids[user_index];
                let journal = journals[user_index];
                let written = at(days_ago);
                let published = at(1);
                conn.batch_execute(&format!("INSERT INTO traces(id,user_id,journal_id,title,subtitle,content,interaction_date,trace_type,status,start_writing_at,finalized_at)
                    VALUES('{trace}','{user}','{journal}','Synthetic','','Test','{written}','USER_TRACE','FINALIZED','{written}','{written}');
                    INSERT INTO posts(id,user_id,source_trace_id,title,subtitle,content,interaction_type,post_type,status,publishing_date)
                    VALUES('{post}','{user}','{trace}','','','','OUTPUT','IDEA','PUBLISHED','{published}');")).unwrap();
                if index < shared {
                    conn.batch_execute(&format!("INSERT INTO post_grants(id,post_id,owner_user_id,grantee_scope,access_level,status)
                        VALUES(uuid_generate_v4(),'{post}','{user}','ALL_PLATFORM_USERS','READ','ACTIVE');")).unwrap();
                }
                if user_index == 0 {
                    publisher_posts.push(post);
                }
            }
        }
        // Duplicate redistributions today must not move the original publication.
        let post = publisher_posts[0];
        let yesterday = at(1);
        conn.batch_execute(&format!(
            "INSERT INTO post_distribution_events(post_id,distributed_at,notification_mode)
            VALUES('{post}','{yesterday}','NONE'),('{post}','{now}','NONE');"
        ))
        .unwrap();
        let (album, document) = (Uuid::new_v4(), Uuid::new_v4());
        let reader = ids[2];
        let inactive_reader = ids[5];
        conn.batch_execute(&format!(
            "INSERT INTO albums(id,owner_user_id,title) VALUES('{album}','{reader}','Synthetic');
            INSERT INTO documents(id,owner_user_id,document_role,content_source)
            VALUES('{document}','{inactive_reader}','REFERENCE','REFERENCE_ONLY');"
        ))
        .unwrap();
        drop(conn);
        create_usage_event(
            reader,
            None,
            UsageEventType::AlbumOpened,
            Some(album),
            None,
            &pool,
        )
        .unwrap();
        create_usage_event(
            inactive_reader,
            None,
            UsageEventType::DocumentOpened,
            Some(document),
            None,
            &pool,
        )
        .unwrap();
        // This user belongs to ALL three overlapping series: three shared
        // traces, a private majority, and recent reading activity.
        create_usage_event(
            ids[1],
            None,
            UsageEventType::PostOpened,
            Some(publisher_posts[0]),
            None,
            &pool,
        )
        .unwrap();
        assert_eq!(
            create_usage_event(
                ids[0],
                None,
                UsageEventType::AlbumOpened,
                Some(album),
                None,
                &pool
            )
            .unwrap_err()
            .status_code,
            401
        );
        assert_eq!(
            create_usage_event(reader, None, UsageEventType::AlbumOpened, None, None, &pool)
                .unwrap_err()
                .status_code,
            400
        );
        let mut conn = pool.get().unwrap();
        let snapshot = || {
            diesel::sql_query(user_segments_sql())
                .bind::<Timestamp, _>(Utc::now().naive_utc())
                .get_result::<UserSegments>(&mut pool.get().unwrap())
                .unwrap()
        };
        let segments = snapshot();
        assert_eq!(
            (
                segments.publishers,
                segments.private_writers,
                segments.readers,
                segments.churners,
                segments.other
            ),
            (1, 3, 1, 1, 1)
        );
        for (metric, expected) in [
            (AdminPlatformAnalyticsMetric::PublishersShare, 2),
            (AdminPlatformAnalyticsMetric::PrivateWritersShare, 4),
            (AdminPlatformAnalyticsMetric::ReadersShare, 3),
        ] {
            let point = load_series(metric, today, today, &mut conn)
                .unwrap()
                .remove(0);
            assert_eq!(point.count, expected);
            assert_eq!(point.total_users, Some(7));
            assert!((point.percentage.unwrap() - 100.0 * expected as f64 / 7.0).abs() < 1e-10);
        }
        let past = at(40).date();
        let point = load_series(
            AdminPlatformAnalyticsMetric::PrivateWritersShare,
            past,
            past,
            &mut conn,
        )
        .unwrap()
        .remove(0);
        assert_eq!(
            (point.count, point.total_users),
            (2, Some(6)),
            "No future accounts or future third traces"
        );
        assert_eq!(
            load_series(
                AdminPlatformAnalyticsMetric::ReadersShare,
                past,
                past,
                &mut conn
            )
            .unwrap()[0]
                .count,
            0
        );
        let empty = at(200).date();
        let point = load_series(
            AdminPlatformAnalyticsMetric::PublishersShare,
            empty,
            empty,
            &mut conn,
        )
        .unwrap()
        .remove(0);
        assert_eq!(
            (point.count, point.total_users, point.percentage),
            (0, Some(0), Some(0.0))
        );
        for metric in [
            AdminPlatformAnalyticsMetric::DocumentOpened,
            AdminPlatformAnalyticsMetric::AlbumOpened,
        ] {
            let points =
                super::super::admin::load_platform_metric_points(metric, today, today, &mut conn)
                    .unwrap();
            assert_eq!(points[0].count, 1);
            assert!(points[0].percentage.is_none());
        }
        let series = |metric, day, conn: &mut PgConnection| {
            load_series(metric, day, day, conn).unwrap()[0].count
        };
        assert_eq!(
            series(
                AdminPlatformAnalyticsMetric::DailyPublishers,
                at(1).date(),
                &mut conn
            ),
            2
        );
        assert_eq!(
            series(
                AdminPlatformAnalyticsMetric::DailyPublishers,
                today,
                &mut conn
            ),
            0
        );
        assert_eq!(
            series(
                AdminPlatformAnalyticsMetric::DailyPrivateWriters,
                at(1).date(),
                &mut conn
            ),
            1
        );
        assert_eq!(
            series(
                AdminPlatformAnalyticsMetric::InactiveWriters30d,
                today,
                &mut conn
            ),
            2
        );
        assert_eq!(
            series(
                AdminPlatformAnalyticsMetric::InactiveWriters60d,
                today,
                &mut conn
            ),
            2
        );
        assert_eq!(
            series(
                AdminPlatformAnalyticsMetric::InactiveWriters30d,
                at(40).date(),
                &mut conn
            ),
            2
        );
        assert_eq!(
            series(
                AdminPlatformAnalyticsMetric::InactiveWriters30d,
                today + chrono::Duration::days(1),
                &mut conn
            ),
            0
        );
        assert_eq!(
            series(
                AdminPlatformAnalyticsMetric::DailyPrivateWriters,
                at(3).date(),
                &mut conn
            ),
            0
        );
        for post in publisher_posts {
            conn.batch_execute(&format!(
                "UPDATE post_grants SET status='REVOKED' WHERE post_id='{post}'"
            ))
            .unwrap();
        }
        assert_eq!(
            series(
                AdminPlatformAnalyticsMetric::DailyPrivateWriters,
                at(3).date(),
                &mut conn
            ),
            1
        );
        assert_eq!(snapshot().publishers, 0);
        // Validate bulk timezone expression, including an unsupported stored name.
        let timezone =
            crate::entities_v2::platform_infra::timezone::postgres_timezone_sql("u.timezone");
        conn.batch_execute(&format!(
            "SELECT timezone({timezone},NOW()) FROM
            (VALUES('Asia/Calcutta'),('Asia/Saigon'),('bad/timezone'),('UTC')) u(timezone)"
        ))
        .unwrap();
        diesel::sql_query("DELETE FROM users WHERE id=ANY($1)")
            .bind::<diesel::sql_types::Array<diesel::sql_types::Uuid>, _>(ids)
            .execute(&mut conn)
            .unwrap();
    }
}
