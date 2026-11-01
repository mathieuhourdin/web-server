use super::{activate_due_automatic_retries, Lens, LensProcessingState, NewLens};
use crate::{
    db::DbPool,
    entities_v2::{
        landscape_analysis::{create_for_trace_and_lens, NewLandscapeAnalysis},
        trace::NewTrace,
    },
};
use diesel::{connection::SimpleConnection, r2d2::ConnectionManager};
use diesel_migrations::MigrationHarness;
use uuid::Uuid;

const MIGRATIONS: diesel_migrations::EmbeddedMigrations = diesel_migrations::embed_migrations!();

#[test]
#[ignore = "Requires an explicitly configured disposable blog_test database"]
fn ai_disabled_planning_is_noop_and_cleanup_preserves_real_failures() {
    let url = std::env::var("BLOG_TEST_DATABASE_URL").unwrap();
    assert_eq!(url::Url::parse(&url).unwrap().path(), "/blog_test");
    let pool = DbPool::builder()
        .max_size(5)
        .build(ConnectionManager::new(url))
        .unwrap();
    pool.get()
        .unwrap()
        .run_pending_migrations(MIGRATIONS)
        .unwrap();
    let owner = Uuid::new_v4();
    let journal = Uuid::new_v4();
    pool.get().unwrap().batch_execute(&format!("INSERT INTO users(id,email,first_name,last_name,handle,password,updated_at,
        ai_features_enabled,ai_features_enabled_by_admin)
        VALUES('{owner}','{owner}@test.invalid','Test','Author','@{owner}','unused',NOW(),FALSE,TRUE);
        INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status)
        VALUES('{journal}','{owner}','Test','','','USER_JOURNAL','ACTIVE');")).unwrap();
    let now = chrono::Utc::now().naive_utc();
    let trace = NewTrace::new(
        "Test".into(),
        "".into(),
        "Synthetic test content".into(),
        now,
        owner,
        journal,
    )
    .create_finalized(&pool)
    .unwrap();
    let new_lens = |target| {
        NewLens {
            user_id: owner,
            target_trace_id: target,
            current_landscape_id: None,
            fork_landscape_id: None,
            autoplay: true,
            processing_state: LensProcessingState::InSync,
        }
        .create(&pool)
        .unwrap()
    };
    let lens = new_lens(Some(trace.id));
    assert_ne!(lens.processing_state, LensProcessingState::Failed);
    assert!(lens.get_analysis_scope_ids(&pool).unwrap().is_empty());
    // The per-trace entry point must return before loading even a trace record.
    assert!(create_for_trace_and_lens(Uuid::new_v4(), lens.id, &pool)
        .unwrap()
        .is_empty());
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE users SET ai_features_enabled=TRUE,
        ai_features_enabled_by_admin=FALSE WHERE id='{owner}'"
        ))
        .unwrap();
    lens.clone()
        .plan_pending_analyses_for_target(&pool)
        .unwrap();
    assert!(create_for_trace_and_lens(Uuid::new_v4(), lens.id, &pool)
        .unwrap()
        .is_empty());
    assert!(lens.get_analysis_scope_ids(&pool).unwrap().is_empty());
    let disabled_reason = "AI features are disabled for this account";
    let lens = lens.set_failed(disabled_reason.into(), &pool).unwrap();
    let real_failure = new_lens(None)
        .set_failed("Provider unavailable".into(), &pool)
        .unwrap();
    let unexplained = new_lens(None)
        .set_processing_state(LensProcessingState::Failed, &pool)
        .unwrap();
    let job = NewLandscapeAnalysis::new_trace_incremental(
        "Test".into(),
        "".into(),
        "".into(),
        owner,
        trace.effective_datetime(),
        None,
        Some(trace.id),
        None,
    )
    .create_for_lens(lens.id, &pool)
    .unwrap();
    job.set_failed(disabled_reason.into(), &pool).unwrap();
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE lenses SET automatic_retry_at=NOW()-INTERVAL '1 minute'
        WHERE id IN('{}','{}');",
            lens.id, real_failure.id
        ))
        .unwrap();
    assert!(activate_due_automatic_retries(&pool).unwrap().is_empty());
    assert_eq!(
        Lens::find_full_lens(real_failure.id, &pool)
            .unwrap()
            .automatic_retry_count,
        0
    );
    pool.get()
        .unwrap()
        .batch_execute(include_str!(
            "../../../../migrations/2026-11-01-150000_clear_ai_disabled_analysis_failures/up.sql"
        ))
        .unwrap();
    let repaired = Lens::find_full_lens(lens.id, &pool).unwrap();
    assert_eq!(repaired.processing_state, LensProcessingState::OutOfSync);
    assert!(repaired.failure_reason.is_none());
    assert!(repaired.automatic_retry_at.is_none());
    assert_eq!(repaired.automatic_retry_count, 0);
    let jobs = repaired.get_analysis_scope(&pool).unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(
        jobs[0].processing_state,
        crate::entities_v2::landscape_analysis::LandscapeProcessingState::Pending
    );
    assert!(jobs[0].failure_reason.is_none());
    assert_eq!(
        Lens::find_full_lens(real_failure.id, &pool)
            .unwrap()
            .failure_reason
            .as_deref(),
        Some("Provider unavailable")
    );
    assert_eq!(
        Lens::find_full_lens(unexplained.id, &pool)
            .unwrap()
            .processing_state,
        LensProcessingState::Failed
    );
    assert!(activate_due_automatic_retries(&pool).unwrap().is_empty());
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE users SET ai_features_enabled_by_admin=TRUE WHERE id='{owner}'"
        ))
        .unwrap();
    assert_eq!(
        activate_due_automatic_retries(&pool).unwrap(),
        vec![real_failure.id]
    );
    assert!(activate_due_automatic_retries(&pool).unwrap().is_empty());
    repaired.plan_pending_analyses_for_target(&pool).unwrap();
    assert!(
        Lens::find_full_lens(lens.id, &pool)
            .unwrap()
            .get_analysis_scope_ids(&pool)
            .unwrap()
            .len()
            > 1
    );
    pool.get()
        .unwrap()
        .batch_execute(&format!("DELETE FROM users WHERE id='{owner}'"))
        .unwrap();
}
