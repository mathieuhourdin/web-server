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

#[tokio::test]
#[ignore = "Requires an explicitly configured disposable blog_test database; makes no LLM calls"]
async fn quota_deferral_resumes_planning_without_failing_or_duplicating_jobs() {
    use super::LensPlanningOutcome;
    use crate::entities_v2::landscape_analysis::model::LandscapeAnalysisType;
    use crate::entities_v2::landscape_analysis::{
        claim_next_pending_for_lens, find_lens_ids_with_pending_analyses, LandscapeProcessingState,
    };
    use crate::entities_v2::trace::Trace;
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
    crate::db::init_global_pool(pool.clone());
    let owner = Uuid::new_v4();
    let journal = Uuid::new_v4();
    pool.get().unwrap().batch_execute(&format!(
        "INSERT INTO users(id,email,first_name,last_name,handle,password,updated_at,is_platform_user,
            ai_features_enabled,ai_features_enabled_by_admin)
         VALUES('{owner}','{owner}@test.invalid','Synthetic','Author','{owner}','unused',NOW(),FALSE,TRUE,TRUE);
         INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status)
         VALUES('{journal}','{owner}','Synthetic','','','USER_JOURNAL','ACTIVE');
         INSERT INTO traces(id,user_id,journal_id,title,subtitle,content,interaction_date,trace_type,status,start_writing_at,finalized_at)
         SELECT gen_random_uuid(),'{owner}','{journal}','Synthetic','','Synthetic',
            NOW()-INTERVAL '40 days'+n*INTERVAL '1 hour','USER_TRACE','FINALIZED',
            NOW()-INTERVAL '40 days'+n*INTERVAL '1 hour',NOW()-INTERVAL '40 days'+n*INTERVAL '1 hour'
         FROM generate_series(1,31) n;"
    )).unwrap();
    let target = Trace::get_all_for_user(owner, &pool)
        .unwrap()
        .into_iter()
        .max_by_key(|trace| trace.effective_datetime())
        .unwrap();
    let lens = NewLens {
        user_id: owner,
        target_trace_id: None,
        current_landscape_id: None,
        fork_landscape_id: None,
        autoplay: true,
        processing_state: LensProcessingState::OutOfSync,
    }
    .create(&pool)
    .unwrap()
    .set_target_trace(Some(target.id), &pool)
    .unwrap();
    // Cron must discover incomplete planning even before the first job exists.
    assert!(find_lens_ids_with_pending_analyses(&pool)
        .unwrap()
        .contains(&lens.id));
    assert_eq!(
        lens.clone()
            .plan_pending_analyses_for_target(&pool)
            .unwrap(),
        LensPlanningOutcome::Deferred
    );
    let scope = lens.get_analysis_scope(&pool).unwrap();
    let trace_jobs = scope
        .iter()
        .filter(|a| a.landscape_analysis_type == LandscapeAnalysisType::TraceIncremental)
        .count();
    assert_eq!(trace_jobs, 30);
    assert!(!scope.iter().any(|a| a.analyzed_trace_id == Some(target.id)));
    let deferred = Lens::find_full_lens(lens.id, &pool).unwrap();
    assert_eq!(deferred.processing_state, LensProcessingState::OutOfSync);
    assert!(deferred.failure_reason.is_none() && deferred.automatic_retry_at.is_none());
    assert_eq!(deferred.automatic_retry_count, 0);
    assert_eq!(
        lens.clone()
            .plan_pending_analyses_for_target(&pool)
            .unwrap(),
        LensPlanningOutcome::Deferred
    );
    assert_eq!(
        lens.get_analysis_scope_ids(&pool).unwrap().len(),
        scope.len()
    );
    // A second planner yields to the same lock used by the execution worker.
    let worker = Uuid::new_v4();
    assert!(lens.try_acquire_run_lock(worker, 60, &pool).unwrap());
    assert_eq!(
        lens.clone()
            .plan_pending_analyses_for_target(&pool)
            .unwrap(),
        LensPlanningOutcome::Deferred
    );
    assert!(lens.release_run_lock(worker, &pool).unwrap());
    // Already queued work remains claimable despite the planning quota.
    let job = claim_next_pending_for_lens(lens.id, chrono::Utc::now().naive_utc(), &pool)
        .unwrap()
        .unwrap();
    assert_eq!(
        job.landscape_analysis_type,
        LandscapeAnalysisType::TraceIncremental
    );
    job.set_processing_state(LandscapeProcessingState::Pending, &pool)
        .unwrap();
    // Simulate completed processing by metadata only. The real worker then has
    // no LLM work to execute, but must not acknowledge the unfinished planning.
    let finish_queued = || {
        pool.get()
            .unwrap()
            .batch_execute(&format!(
                "UPDATE landscape_analyses SET processing_state='COMPLETED'
         WHERE id IN (SELECT landscape_analysis_id FROM lens_analysis_scopes WHERE lens_id='{}')",
                lens.id
            ))
            .unwrap()
    };
    finish_queued();
    let resumed = crate::work_analyzer::run_lens(lens.id).await.unwrap();
    assert_eq!(resumed.processing_state, LensProcessingState::OutOfSync);
    assert_eq!(resumed.automatic_retry_count, 0);
    assert_eq!(
        crate::work_analyzer::run_lens_one(lens.id)
            .await
            .unwrap()
            .processing_state,
        LensProcessingState::OutOfSync
    );
    // Let the first batch leave the rolling window; the next pass adds only
    // the missing trace job, rather than duplicating the first thirty.
    pool.get()
        .unwrap()
        .batch_execute(&format!(
        "UPDATE landscape_analyses SET created_at=NOW()-INTERVAL '25 hours' WHERE user_id='{owner}'"
    ))
        .unwrap();
    assert_eq!(
        lens.clone()
            .plan_pending_analyses_for_target(&pool)
            .unwrap(),
        LensPlanningOutcome::Complete
    );
    let scope = lens.get_analysis_scope(&pool).unwrap();
    let trace_ids = scope
        .iter()
        .filter_map(|a| a.analyzed_trace_id)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(trace_ids.len(), 31);
    assert_eq!(
        scope
            .iter()
            .filter(|a| a.analyzed_trace_id.is_some())
            .count(),
        31
    );
    finish_queued();
    let finished = crate::work_analyzer::run_lens(lens.id).await.unwrap();
    assert_eq!(finished.processing_state, LensProcessingState::InSync);
    assert!(!find_lens_ids_with_pending_analyses(&pool)
        .unwrap()
        .contains(&lens.id));
    // A stale worker cannot mark a newer target in sync or clear a real failure.
    let new_target = NewTrace::new(
        "Synthetic".into(),
        "".into(),
        "Synthetic".into(),
        chrono::Utc::now().naive_utc(),
        owner,
        journal,
    )
    .create_finalized(&pool)
    .unwrap();
    finished
        .clone()
        .set_target_trace(Some(new_target.id), &pool)
        .unwrap();
    let current = finished
        .finish_run_for_target(Some(target.id), LensProcessingState::InSync, &pool)
        .unwrap();
    assert_eq!(current.processing_state, LensProcessingState::OutOfSync);
    let failed = current
        .set_failed("Provider unavailable".into(), &pool)
        .unwrap()
        .finish_run_for_target(Some(new_target.id), LensProcessingState::InSync, &pool)
        .unwrap();
    assert_eq!(failed.processing_state, LensProcessingState::Failed);
    assert_eq!(
        failed.failure_reason.as_deref(),
        Some("Provider unavailable")
    );
    assert!(!find_lens_ids_with_pending_analyses(&pool)
        .unwrap()
        .contains(&lens.id));
    pool.get()
        .unwrap()
        .batch_execute(&format!("DELETE FROM users WHERE id='{owner}'"))
        .unwrap();
}

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
