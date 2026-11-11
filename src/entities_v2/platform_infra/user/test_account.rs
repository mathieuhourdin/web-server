//! Admin classification only; this flag is not an authorization boundary.
use super::admin::ensure_admin_session_user;
use crate::{
    db::DbPool,
    entities_v2::{error::PpdcError, session::Session},
    schema::users,
};
use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestAccountInput {
    pub is_test_account: bool,
}

#[derive(Serialize)]
pub struct TestAccountResponse {
    pub id: Uuid,
    pub is_test_account: bool,
}

fn set_test_account(
    id: Uuid,
    value: bool,
    pool: &DbPool,
) -> Result<TestAccountResponse, PpdcError> {
    let (id, is_test_account) = diesel::update(users::table.find(id))
        .set((
            users::is_test_account.eq(value),
            users::updated_at.eq(chrono::Utc::now().naive_utc()),
        ))
        .returning((users::id, users::is_test_account))
        .get_result::<(Uuid, bool)>(&mut pool.get()?)?;
    Ok(TestAccountResponse {
        id,
        is_test_account,
    })
}

#[debug_handler]
pub async fn patch(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(input): Json<TestAccountInput>,
) -> Result<Json<TestAccountResponse>, PpdcError> {
    tokio::task::spawn_blocking(move || {
        ensure_admin_session_user(&session, &pool)?;
        set_test_account(id, input.is_test_account, &pool).map(Json)
    })
    .await
    .map_err(|_| {
        PpdcError::new(
            500,
            crate::entities_v2::error::ErrorType::InternalError,
            "Test account update failed".into(),
        )
    })?
}

#[cfg(test)]
mod tests {
    use super::super::{
        admin, analytics_devices, analytics_engagement, analytics_nudges, routes, User,
        UserListParams, UserSearchParams,
    };
    use super::*;
    use axum::extract::Query;
    use diesel::{connection::SimpleConnection, r2d2::ConnectionManager};
    use diesel_migrations::{embed_migrations, MigrationHarness};
    use serde_json::json;

    #[tokio::test]
    #[ignore = "requires a disposable TEST_ACCOUNTS_DATABASE_URL database"]
    async fn admin_test_account_classification_filters_discovery_and_analytics() {
        let url = std::env::var("TEST_ACCOUNTS_DATABASE_URL").unwrap();
        assert_eq!(url::Url::parse(&url).unwrap().path(), "/test_accounts_test");
        let pool = DbPool::builder()
            .max_size(4)
            .build(ConnectionManager::new(url))
            .unwrap();
        const MIGRATIONS: diesel_migrations::EmbeddedMigrations = embed_migrations!();
        pool.get()
            .unwrap()
            .run_pending_migrations(MIGRATIONS)
            .unwrap();
        let (admin_id, member, target, trace) = (
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        );
        let name = format!("probe{}", target.simple());
        let journal = Uuid::new_v4();
        pool.get().unwrap().batch_execute(&format!("
            INSERT INTO users(id,email,first_name,last_name,handle,password,is_platform_user)
            VALUES('{admin_id}','{admin_id}@test.invalid','Admin','Test','{admin_id}','unused',TRUE),
                  ('{member}','{member}@test.invalid','Member','Test','{member}','unused',TRUE),
                  ('{target}','{target}@test.invalid','{name}','','{name}','unchanged-password',TRUE);
            INSERT INTO user_roles(user_id,role) VALUES('{admin_id}','ADMIN');
            INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status)
            VALUES('{journal}','{target}','Synthetic','','','USER_JOURNAL','ACTIVE');
            INSERT INTO traces(id,user_id,journal_id,title,subtitle,content,trace_type,status,start_writing_at,finalized_at)
            VALUES('{trace}','{target}','{journal}','Synthetic','','Synthetic','USER_TRACE','FINALIZED',NOW(),NOW());
        ")).unwrap();
        let mut admin_session = Session::anonymous();
        admin_session.user_id = Some(admin_id);
        let mut member_session = Session::anonymous();
        member_session.user_id = Some(member);
        let query = || serde_json::from_value::<UserSearchParams>(json!({"q":name})).unwrap();
        assert_eq!(
            routes::get_user_search_route(
                Query(query()),
                Extension(pool.clone()),
                Extension(member_session.clone())
            )
            .await
            .unwrap()
            .0
            .total,
            1
        );
        let day = chrono::Utc::now().date_naive();
        let metric = admin::AdminPlatformAnalyticsMetric::WrittenTraces;
        let before = admin::load_platform_metric_points(metric, day, day, &mut pool.get().unwrap())
            .unwrap()[0]
            .count;
        assert_eq!(
            patch(
                Extension(pool.clone()),
                Extension(member_session.clone()),
                Path(target),
                Json(TestAccountInput {
                    is_test_account: true
                })
            )
            .await
            .err()
            .unwrap()
            .status_code,
            403
        );
        assert_eq!(
            patch(
                Extension(pool.clone()),
                Extension(Session::anonymous()),
                Path(target),
                Json(TestAccountInput {
                    is_test_account: true
                })
            )
            .await
            .err()
            .unwrap()
            .status_code,
            401
        );
        for _ in 0..2 {
            assert!(
                patch(
                    Extension(pool.clone()),
                    Extension(admin_session.clone()),
                    Path(target),
                    Json(TestAccountInput {
                        is_test_account: true
                    })
                )
                .await
                .unwrap()
                .0
                .is_test_account
            );
        }
        assert_eq!(
            routes::get_user_search_route(
                Query(query()),
                Extension(pool.clone()),
                Extension(member_session.clone())
            )
            .await
            .unwrap()
            .0
            .total,
            0
        );
        let listing = routes::get_users(
            Query(serde_json::from_value::<UserListParams>(json!({})).unwrap()),
            Extension(pool.clone()),
            Extension(member_session.clone()),
        )
        .await
        .unwrap()
        .0;
        assert!(!listing.items.iter().any(|u| u.id == target));
        let admin_query = serde_json::from_value(json!({"q":name,"is_test_account":true})).unwrap();
        let listing = admin::get_admin_users_route(
            Extension(pool.clone()),
            Extension(admin_session.clone()),
            Query(admin_query),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(listing.total, 1);
        assert!(listing.items[0].is_test_account);
        let user = User::find(&target, &pool).unwrap();
        assert!(user.is_test_account);
        assert_eq!(user.password, "unchanged-password");
        assert!(serde_json::to_value(&user)
            .unwrap()
            .get("is_test_account")
            .is_none());
        assert_eq!(
            admin::load_platform_metric_points(metric, day, day, &mut pool.get().unwrap()).unwrap()
                [0]
            .count,
            before - 1
        );
        // Exercise all SQL variants against the same small fixture, not production.
        for name in [
            "users",
            "published_posts",
            "followed_journal_opened",
            "feed_visited",
            "feed_engaged_30s",
            "post_opened",
            "document_opened",
            "album_opened",
            "writing_nudge_shown",
            "writing_nudge_opened",
            "writing_nudge_dismissed",
            "writing_nudge_shown_users",
            "writing_nudge_opened_users",
            "writing_nudge_dismissed_users",
            "daily_publishers",
            "daily_private_writers",
            "publishers_share",
            "private_writers_share",
            "readers_share",
            "inactive_writers_30d",
            "inactive_writers_60d",
            "active_writers_30d",
            "active_writers_14d",
            "active_readers_30d",
            "active_readers_7d",
        ] {
            let metric = serde_json::from_value(json!(name)).unwrap();
            admin::load_platform_metric_points(metric, day, day, &mut pool.get().unwrap()).unwrap();
        }
        let _ = admin::get_admin_analytics_summary_route(
            Extension(pool.clone()),
            Extension(admin_session.clone()),
        )
        .await
        .unwrap();
        let _ = admin::get_admin_platform_overview_route(
            Extension(pool.clone()),
            Extension(admin_session.clone()),
        )
        .await
        .unwrap();
        let analytic_users = admin::get_admin_analytics_users_route(
            Extension(pool.clone()),
            Extension(admin_session.clone()),
            Query(serde_json::from_value(json!({})).unwrap()),
        )
        .await
        .unwrap()
        .0;
        assert!(!analytic_users.items.iter().any(|u| u.id == target));
        let recent = admin::get_admin_recent_user_activity_route(
            Extension(pool.clone()),
            Extension(admin_session.clone()),
            Query(serde_json::from_value(json!({})).unwrap()),
        )
        .await
        .unwrap()
        .0;
        assert!(!recent.items.iter().any(|u| u.id == target));
        let _ = analytics_devices::get_admin_device_analytics_route(
            Extension(pool.clone()),
            Extension(admin_session.clone()),
            Query(serde_json::from_value(json!({})).unwrap()),
        )
        .await
        .unwrap();
        let _ = analytics_engagement::get_admin_user_segments_route(
            Extension(pool.clone()),
            Extension(admin_session.clone()),
        )
        .await
        .unwrap();
        let _ = analytics_nudges::get_admin_writing_nudges_route(
            Extension(pool.clone()),
            Extension(admin_session.clone()),
            Query(serde_json::from_value(json!({})).unwrap()),
        )
        .await
        .unwrap();
        assert_eq!(
            patch(
                Extension(pool.clone()),
                Extension(admin_session.clone()),
                Path(Uuid::new_v4()),
                Json(TestAccountInput {
                    is_test_account: true
                })
            )
            .await
            .err()
            .unwrap()
            .status_code,
            404
        );
        assert!(
            !patch(
                Extension(pool.clone()),
                Extension(admin_session),
                Path(target),
                Json(TestAccountInput {
                    is_test_account: false
                })
            )
            .await
            .unwrap()
            .0
            .is_test_account
        );
        assert_eq!(
            routes::get_user_search_route(
                Query(query()),
                Extension(pool.clone()),
                Extension(member_session)
            )
            .await
            .unwrap()
            .0
            .total,
            1
        );
        assert_eq!(
            admin::load_platform_metric_points(metric, day, day, &mut pool.get().unwrap()).unwrap()
                [0]
            .count,
            before
        );
        diesel::delete(crate::schema::traces::table.find(trace))
            .execute(&mut pool.get().unwrap())
            .unwrap();
        diesel::delete(crate::schema::journals::table.find(journal))
            .execute(&mut pool.get().unwrap())
            .unwrap();
        diesel::delete(users::table.filter(users::id.eq_any([admin_id, member, target])))
            .execute(&mut pool.get().unwrap())
            .unwrap();
    }
}
