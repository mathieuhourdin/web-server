use axum::{
    extract::Extension,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use diesel::prelude::*;
use std::time::Duration;

use crate::db::DbPool;

/// Public readiness probe: no user records, credentials or schema details.
pub async fn get_healthz(Extension(pool): Extension<DbPool>) -> Response {
    // Diesel is synchronous: don't block an async worker or wait for the pool's
    // normal 30-second timeout just to answer a health check.
    let probe = tokio::task::spawn_blocking(move || {
        let mut conn = pool
            .get_timeout(Duration::from_secs(2))
            .map_err(|_| "pool_unavailable")?;
        conn.build_transaction()
            .read_only()
            .run::<_, diesel::result::Error, _>(|conn| {
                diesel::sql_query("SET LOCAL statement_timeout='1s'").execute(conn)?;
                diesel::sql_query("SELECT 1").execute(conn)?;
                Ok(())
            })
            .map_err(|_| "query_failed")
    });
    let failure = match tokio::time::timeout(Duration::from_secs(4), probe).await {
        Ok(Ok(Ok(()))) => None,
        Ok(Ok(Err(reason))) => Some(reason),
        Ok(Err(_)) => Some("probe_worker_failed"),
        Err(_) => Some("probe_timeout"),
    };
    let (status, state) = if let Some(reason) = failure {
        tracing::warn!(target: "health", reason, "database_health_check_failed");
        (StatusCode::SERVICE_UNAVAILABLE, "unavailable")
    } else {
        (StatusCode::OK, "ok")
    };
    (
        status,
        [(header::CACHE_CONTROL, "no-store")],
        Json(serde_json::json!({"status":state,"database":state})),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::r2d2::ConnectionManager;

    #[tokio::test]
    #[ignore = "Requires an explicitly configured disposable health_test database"]
    async fn healthz_checks_database_and_reports_exhaustion_without_details() {
        let url = std::env::var("HEALTH_TEST_DATABASE_URL").unwrap();
        assert_eq!(url::Url::parse(&url).unwrap().path(), "/health_test");
        let pool = DbPool::builder()
            .max_size(1)
            .build(ConnectionManager::new(url))
            .unwrap();
        let healthy = get_healthz(Extension(pool.clone())).await;
        assert_eq!(healthy.status(), StatusCode::OK);
        assert_eq!(healthy.headers()[header::CACHE_CONTROL], "no-store");
        let body = axum::body::to_bytes(healthy.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            serde_json::json!({"status":"ok","database":"ok"})
        );
        let held = pool.get().unwrap();
        let unavailable = get_healthz(Extension(pool.clone())).await;
        assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = axum::body::to_bytes(unavailable.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            serde_json::json!({"status":"unavailable","database":"unavailable"})
        );
        drop(held);
        assert_eq!(get_healthz(Extension(pool)).await.status(), StatusCode::OK);
    }
}
