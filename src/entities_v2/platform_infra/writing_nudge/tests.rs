use super::*;
use diesel::{connection::SimpleConnection, r2d2::ConnectionManager};
use diesel_migrations::MigrationHarness;

#[test]
fn local_midnight_and_plain_text_are_safe() {
    let paris = "Europe/Paris".parse().unwrap();
    for (now, expires) in [
        ("2026-03-28T23:30:00Z", "2026-03-29T22:00:00Z"),
        ("2026-10-24T22:30:00Z", "2026-10-25T23:00:00Z"),
    ] {
        assert_eq!(
            next_midnight(now.parse().unwrap(), paris).unwrap(),
            expires.parse::<DateTime<Utc>>().unwrap()
        );
    }
    assert_eq!(
        excerpt("# A heading\n\nAn im**port**ant [idea](https://example.com)."),
        "A heading An important idea."
    );
    assert_eq!(excerpt("![photo](https://example.com/a.jpg)"), "photo");
    assert!(excerpt(&"é".repeat(500)).chars().count() <= 240);
    assert_eq!(
        [
            dismissal_days(1),
            dismissal_days(2),
            dismissal_days(3),
            dismissal_days(4)
        ],
        [7, 14, 30, 30]
    );
}

#[test]
fn event_contract_and_database_names_are_explicit() {
    for event in [NudgeEvent::Shown, NudgeEvent::Dismissed, NudgeEvent::Opened] {
        assert_eq!(NudgeEvent::from_db(event.to_db()), Some(event));
    }
    for kind in NudgeType::ALL {
        assert_eq!(NudgeType::from_db(kind.to_db()), Some(kind));
    }
    assert!(
        serde_json::from_str::<super::super::model::EventInput>(r#"{"event":"shown"}"#).is_ok()
    );
    assert!(
        serde_json::from_str::<super::super::model::EventInput>(r#"{"event":"SHOWN"}"#).is_err()
    );
    assert!(serde_json::from_str::<super::super::model::EventInput>(
        r#"{"event":"shown","user_id":"spoof"}"#
    )
    .is_err());
    assert_eq!(
        serde_json::to_value(CurrentNudge { nudge: None }).unwrap(),
        serde_json::json!({"nudge":null})
    );
}

struct Fixture {
    pool: DbPool,
    now: DateTime<Utc>,
    users: Vec<Uuid>,
}

impl Fixture {
    fn nudge_event_count(&self, user: Uuid, id: Uuid) -> i64 {
        use crate::schema::usage_events::dsl::{resource_id, usage_events, user_id};
        usage_events
            .filter(user_id.eq(user))
            .filter(resource_id.eq(id))
            .count()
            .get_result(&mut self.pool.get().unwrap())
            .unwrap()
    }

    fn new() -> Self {
        let url = std::env::var("BLOG_TEST_DATABASE_URL").expect("Use disposable blog_test only");
        assert_eq!(url::Url::parse(&url).unwrap().path(), "/blog_test");
        let pool = DbPool::builder()
            .max_size(5)
            .build(ConnectionManager::new(url))
            .unwrap();
        const MIGRATIONS: diesel_migrations::EmbeddedMigrations =
            diesel_migrations::embed_migrations!();
        pool.get()
            .unwrap()
            .run_pending_migrations(MIGRATIONS)
            .unwrap();
        Self {
            pool,
            now: "2026-11-10T12:00:00Z".parse().unwrap(),
            users: vec![],
        }
    }

    fn user(&mut self) -> Uuid {
        let user = Uuid::new_v4();
        self.pool.get().unwrap().batch_execute(&format!(
            "INSERT INTO users(id,email,first_name,last_name,handle,password,updated_at,timezone,
                ai_features_enabled,ai_features_enabled_by_admin)
             VALUES('{user}','{user}@test.invalid','Synthetic','Test','{user}','unused',NOW(),'Europe/Paris',FALSE,FALSE)"
        )).unwrap();
        self.users.push(user);
        user
    }

    fn trace(&self, user: Uuid, age: i64, status: &str) -> Uuid {
        let trace = Uuid::new_v4();
        let journal = Uuid::new_v4();
        self.pool
            .get()
            .unwrap()
            .batch_execute(&format!(
                "INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status)
             VALUES('{journal}','{user}','Synthetic','','','USER_JOURNAL','ACTIVE')"
            ))
            .unwrap();
        let at = (self.now - Duration::days(age)).naive_utc();
        diesel::sql_query(
            "INSERT INTO traces(id,user_id,journal_id,title,subtitle,content,interaction_date,
            trace_type,status,start_writing_at,finalized_at,created_at,updated_at,is_blank)
            VALUES($1,$2,$3,'Synthetic','','# An **idea**',$4,'USER_TRACE',$5,$4,
                CASE WHEN $5='DRAFT' THEN NULL ELSE $4 END,$4,$4,FALSE)",
        )
        .bind::<SqlUuid, _>(trace)
        .bind::<SqlUuid, _>(user)
        .bind::<SqlUuid, _>(journal)
        .bind::<Timestamp, _>(at)
        .bind::<Text, _>(status)
        .execute(&mut self.pool.get().unwrap())
        .unwrap();
        trace
    }

    fn state(&self, user: Uuid, kind: NudgeType) -> State {
        states(user, &mut self.pool.get().unwrap())
            .unwrap()
            .into_iter()
            .find(|s| s.kind().unwrap() == kind)
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for user in &self.users {
            self.pool
                .get()
                .unwrap()
                .batch_execute(&format!("DELETE FROM users WHERE id='{user}'"))
                .unwrap();
        }
    }
}

#[test]
#[ignore = "Requires explicitly configured disposable blog_test; no LLMs or external services"]
fn lifecycle_eligibility_privacy_and_concurrent_tracking() {
    let mut f = Fixture::new();
    let user = f.user();
    let foreign = f.user();
    let first = current(user, f.now, &f.pool).unwrap().nudge.unwrap();
    assert_eq!(first.kind, NudgeType::FirstTrace);
    assert!(first.trace.is_none());
    assert_eq!(
        current(user, f.now, &f.pool).unwrap().nudge.unwrap().id,
        first.id
    );
    assert!(f.state(user, NudgeType::FirstTrace).last_shown_at.is_none());
    assert_eq!(f.nudge_event_count(user, first.id), 0);
    // Analytics and cooldown state must either both persist or both roll back.
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "ALTER TABLE usage_events ADD CONSTRAINT synthetic_nudge_insert_failure
         CHECK (user_id IS DISTINCT FROM '{user}'::uuid OR event_type<>'WRITING_NUDGE_SHOWN')"
        ))
        .unwrap();
    let failed = record_event(user, first.id, NudgeEvent::Shown, f.now, &f.pool);
    f.pool
        .get()
        .unwrap()
        .batch_execute("ALTER TABLE usage_events DROP CONSTRAINT synthetic_nudge_insert_failure")
        .unwrap();
    assert!(failed.is_err());
    assert!(f
        .state(user, NudgeType::FirstTrace)
        .first_shown_at
        .is_none());
    assert_eq!(f.nudge_event_count(user, first.id), 0);
    for event in [
        UsageEventType::WritingNudgeShown,
        UsageEventType::WritingNudgeOpened,
        UsageEventType::WritingNudgeDismissed,
    ] {
        assert!(crate::entities_v2::usage_event::create_usage_event(
            user,
            None,
            event,
            Some(first.id),
            None,
            &f.pool
        )
        .is_err());
    }
    // Fetching yesterday but never exposing the card does not consume a cooldown.
    let now = f.now + Duration::days(1);
    let first = current(user, now, &f.pool).unwrap().nudge.unwrap();
    record_event(foreign, first.id, NudgeEvent::Dismissed, now, &f.pool).unwrap();
    assert_eq!(f.state(user, NudgeType::FirstTrace).dismissal_count, 0);
    assert_eq!(f.nudge_event_count(foreign, first.id), 0);
    record_event(user, first.id, NudgeEvent::Shown, now, &f.pool).unwrap();
    record_event(
        user,
        first.id,
        NudgeEvent::Shown,
        now + Duration::minutes(5),
        &f.pool,
    )
    .unwrap();
    assert_eq!(
        f.state(user, NudgeType::FirstTrace).first_shown_at,
        Some(now)
    );
    assert_eq!(f.nudge_event_count(user, first.id), 1);
    assert_eq!(
        current(user, now + Duration::minutes(5), &f.pool)
            .unwrap()
            .nudge
            .unwrap()
            .id,
        first.id
    );
    record_event(
        user,
        first.id,
        NudgeEvent::Opened,
        now + Duration::minutes(5),
        &f.pool,
    )
    .unwrap();
    record_event(
        user,
        first.id,
        NudgeEvent::Dismissed,
        now + Duration::minutes(6),
        &f.pool,
    )
    .unwrap();
    assert_eq!(f.state(user, NudgeType::FirstTrace).dismissal_count, 0);
    assert_eq!(f.nudge_event_count(user, first.id), 2);
    assert!(current(user, now + Duration::days(3), &f.pool)
        .unwrap()
        .nudge
        .is_none());
    let now = now + Duration::days(3) + Duration::minutes(6);
    // Concurrent screens get one global reservation, without marking it shown.
    let ids = std::thread::scope(|scope| {
        (0..6)
            .map(|_| scope.spawn(|| current(user, now, &f.pool).unwrap().nudge.unwrap().id))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(ids.iter().all(|id| *id == ids[0]));
    let id = ids[0];
    // Opening/dismissal may arrive before exposure. Duplicate dismissals are inert.
    std::thread::scope(|scope| {
        for _ in 0..6 {
            scope.spawn(|| record_event(user, id, NudgeEvent::Dismissed, now, &f.pool).unwrap());
        }
    });
    assert_eq!(f.state(user, NudgeType::FirstTrace).dismissal_count, 1);
    assert_eq!(f.nudge_event_count(user, id), 2); // implicit exposure + one dismissal, even concurrently.
    assert_eq!(
        f.state(user, NudgeType::FirstTrace).dismissed_until,
        Some(now + Duration::days(7))
    );
    assert!(current(user, now + Duration::days(6), &f.pool)
        .unwrap()
        .nudge
        .is_none());
    let later = now + Duration::days(7);
    let next = current(user, later, &f.pool).unwrap().nudge.unwrap();
    record_event(user, id, NudgeEvent::Dismissed, later, &f.pool).unwrap();
    assert_eq!(
        f.state(user, NudgeType::FirstTrace).suggestion_id,
        Some(next.id)
    );
    record_event(user, next.id, NudgeEvent::Dismissed, later, &f.pool).unwrap();
    assert_eq!(
        f.state(user, NudgeType::FirstTrace).dismissed_until,
        Some(later + Duration::days(14))
    );
    let third = current(user, later + Duration::days(14), &f.pool)
        .unwrap()
        .nudge
        .unwrap();
    record_event(
        user,
        third.id,
        NudgeEvent::Dismissed,
        later + Duration::days(14),
        &f.pool,
    )
    .unwrap();
    assert_eq!(f.state(user, NudgeType::FirstTrace).dismissal_count, 3);

    let draft_user = f.user();
    let old = f.trace(draft_user, 8, "DRAFT");
    let recent = f.trace(draft_user, 4, "DRAFT");
    let blank = f.trace(draft_user, 30, "DRAFT");
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET is_blank=TRUE WHERE id='{blank}'"
        ))
        .unwrap();
    let draft = current(draft_user, f.now, &f.pool).unwrap().nudge.unwrap();
    assert_eq!(draft.kind, NudgeType::ResumeDraft);
    assert_eq!(draft.trace.unwrap().id, recent);
    record_event(draft_user, draft.id, NudgeEvent::Shown, f.now, &f.pool).unwrap();
    // Privacy changes invalidate the preview and must not replace an exposed card.
    f.pool.get().unwrap().batch_execute(&format!(
        "UPDATE journals SET is_encrypted=TRUE WHERE id=(SELECT journal_id FROM traces WHERE id='{recent}')"
    )).unwrap();
    assert!(current(draft_user, f.now, &f.pool).unwrap().nudge.is_none());
    assert_eq!(
        current(draft_user, f.now + Duration::days(3), &f.pool)
            .unwrap()
            .nudge
            .unwrap()
            .trace
            .unwrap()
            .id,
        old
    );

    let memory_user = f.user();
    let old = f.trace(memory_user, 40, "FINALIZED");
    f.trace(memory_user, 13, "FINALIZED");
    assert!(current(memory_user, f.now, &f.pool)
        .unwrap()
        .nudge
        .is_none());
    let memory = current(memory_user, f.now + Duration::days(1), &f.pool)
        .unwrap()
        .nudge
        .unwrap();
    assert_eq!(memory.kind, NudgeType::WritingMemory);
    assert_eq!(memory.trace.unwrap().id, old);
    let imported = f.trace(memory_user, 300, "FINALIZED");
    diesel::sql_query(
        "UPDATE traces SET is_historical_import=TRUE,created_at=$2,updated_at=$2 WHERE id=$1",
    )
    .bind::<SqlUuid, _>(imported)
    .bind::<Timestamp, _>(f.now.naive_utc())
    .execute(&mut f.pool.get().unwrap())
    .unwrap();
    assert!(current(memory_user, f.now + Duration::days(1), &f.pool)
        .unwrap()
        .nudge
        .is_none());

    let encrypted_user = f.user();
    let encrypted = f.trace(encrypted_user, 20, "FINALIZED");
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET is_encrypted=TRUE,encryption_metadata='{{}}' WHERE id='{encrypted}'"
        ))
        .unwrap();
    assert!(current(encrypted_user, f.now, &f.pool)
        .unwrap()
        .nudge
        .is_none());
    // A meaningful recent draft suppresses the first-writing invitation too.
    let active_user = f.user();
    f.trace(active_user, 1, "DRAFT");
    assert!(current(active_user, f.now, &f.pool)
        .unwrap()
        .nudge
        .is_none());
    assert_eq!(f.users.len(), 6);
}
