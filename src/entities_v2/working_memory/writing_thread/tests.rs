use super::persist;
use crate::{
    db::DbPool,
    entities_v2::trace::{NewTrace, TraceStatus},
};
use diesel::{connection::SimpleConnection, r2d2::ConnectionManager};
use diesel_migrations::MigrationHarness;
use uuid::Uuid;

const MIGRATIONS: diesel_migrations::EmbeddedMigrations = diesel_migrations::embed_migrations!();

#[test]
#[ignore = "Requires an explicitly configured disposable blog_test database"]
fn cross_journal_continuations_keep_one_thread_and_filter_all_members() {
    let url = std::env::var("BLOG_TEST_DATABASE_URL").expect("Set a disposable blog_test URL");
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
    let other = Uuid::new_v4();
    let journals = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    let foreign_journal = Uuid::new_v4();
    let mut conn = pool.get().unwrap();
    conn.batch_execute(&format!(
        "INSERT INTO users(id,email,first_name,last_name,handle,password,updated_at)
        VALUES('{owner}','{owner}@test.invalid','Author','Test','@{owner}','unused',NOW()),
              ('{other}','{other}@test.invalid','Other','Test','@{other}','unused',NOW());"
    ))
    .unwrap();
    for (id, user) in journals
        .iter()
        .map(|id| (*id, owner))
        .chain([(foreign_journal, other)])
    {
        conn.batch_execute(&format!(
            "INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status)
            VALUES('{id}','{user}','Test','','','USER_JOURNAL','ACTIVE');"
        ))
        .unwrap();
    }
    drop(conn);
    let make = |user, journal| {
        NewTrace::new(
            "Synthetic trace".into(),
            "".into(),
            "Some synthetic content for a cross-journal continuation.".into(),
            chrono::Utc::now().naive_utc(),
            user,
            journal,
        )
    };
    let a = make(owner, journals[0]).create_finalized(&pool).unwrap();
    let mut b = make(owner, journals[1]).create_finalized(&pool).unwrap();
    b.continues_trace_id = Some(a.id);
    let b = b.update(&pool).unwrap();
    let mut c = make(owner, journals[0]).create_finalized(&pool).unwrap();
    c.continues_trace_id = Some(b.id);
    let c = c.update(&pool).unwrap();
    let all = persist::list(owner, None, None, "", 0, 20, &pool).unwrap();
    assert_eq!(all.1, 1);
    let thread = all.0[0].thread.id;
    assert_eq!(all.0[0].trace_count, 3);
    for journal in &journals[..2] {
        let (items, total) = persist::list(owner, Some(*journal), None, "", 0, 20, &pool).unwrap();
        assert_eq!(total, 1);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].thread.id, thread);
        assert_eq!(items[0].journal_id, Some(journals[0]));
    }
    let (items, total) = persist::list(owner, Some(journals[0]), None, "", 1, 1, &pool).unwrap();
    assert_eq!(total, 1);
    assert!(items.is_empty());
    assert_eq!(
        persist::list(owner, Some(journals[2]), None, "", 0, 20, &pool)
            .unwrap()
            .1,
        0
    );
    let mut draft = make(owner, journals[2]).create(&pool).unwrap();
    draft.continues_trace_id = Some(c.id);
    let draft = draft.update(&pool).unwrap();
    assert_eq!(
        persist::list(owner, Some(journals[2]), None, "", 0, 20, &pool)
            .unwrap()
            .1,
        1
    );
    let d = draft.set_status(TraceStatus::Finalized, &pool).unwrap();
    assert_eq!(persist::view(owner, thread, &pool).unwrap().trace_count, 4);
    let (members, total) = persist::traces(owner, thread, 0, 20, &pool).unwrap();
    assert_eq!(total, 4);
    assert_eq!(
        members.iter().map(|t| t.id).collect::<Vec<_>>(),
        vec![a.id, b.id, c.id, d.id]
    );
    b.set_status(TraceStatus::Archived, &pool).unwrap();
    assert_eq!(
        persist::list(owner, Some(journals[1]), None, "", 0, 20, &pool)
            .unwrap()
            .1,
        1
    );
    assert_eq!(
        persist::list(other, None, None, "", 0, 20, &pool)
            .unwrap()
            .1,
        0
    );
    let mut foreign = make(other, foreign_journal)
        .create_finalized(&pool)
        .unwrap();
    foreign.continues_trace_id = Some(a.id);
    assert!(foreign.update(&pool).is_err());
    let mut cycle = a;
    cycle.continues_trace_id = Some(d.id);
    assert!(cycle.update(&pool).is_err());
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "DELETE FROM users WHERE id IN('{owner}','{other}')"
        ))
        .unwrap();
}
