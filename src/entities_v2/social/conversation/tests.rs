use super::{model::*, service, trace_sources};
use crate::{
    db::DbPool,
    entities_v2::{
        message::{Message, MessageReaction, NewMessage, NewMessageDto},
        session::Session,
        trace::{NewTrace, Trace},
    },
    pagination::ValidatedPagination,
};
use axum::extract::{Extension, Path};
use chrono::Utc;
use diesel::{connection::SimpleConnection, prelude::*, r2d2::ConnectionManager};
use diesel_migrations::{embed_migrations, MigrationHarness};
use uuid::Uuid;

fn page() -> ValidatedPagination {
    ValidatedPagination {
        limit: 100,
        offset: 0,
    }
}
fn session(user: Uuid) -> Session {
    let mut s = Session::anonymous();
    s.user_id = Some(user);
    s
}
fn group(actor: Uuid, others: Vec<Uuid>, pool: &DbPool) -> ConversationView {
    service::create(
        actor,
        CreateConversation {
            kind: ConversationKind::Group,
            title: Some("Synthetic group".into()),
            member_user_ids: others,
        },
        pool,
    )
    .unwrap()
}
fn send(actor: Uuid, id: Uuid, content: &str, pool: &DbPool) -> Message {
    service::send(
        actor,
        id,
        serde_json::from_value(serde_json::json!({"content":content})).unwrap(),
        pool,
    )
    .unwrap()
}
fn direct(a: Uuid, b: Uuid, text: &str, pool: &DbPool) -> Message {
    let input: NewMessageDto =
        serde_json::from_value(serde_json::json!({"recipient_user_id":b,"content":text})).unwrap();
    NewMessage::new(input, a).create(pool).unwrap()
}

#[test]
fn conversation_contract_and_source_update_semantics() {
    for (kind, db) in [
        (ConversationKind::Direct, "DIRECT"),
        (ConversationKind::Group, "GROUP"),
    ] {
        assert_eq!(kind.to_db(), db);
        assert_eq!(ConversationKind::from_db(db).unwrap(), kind);
        assert_eq!(serde_json::to_value(kind).unwrap(), db.to_ascii_lowercase());
    }
    for role in [MemberRole::Admin, MemberRole::Member] {
        assert_eq!(MemberRole::from_db(role.to_db()).unwrap(), role);
    }
    let patch: crate::entities_v2::trace::PatchTraceDto =
        serde_json::from_value(serde_json::json!({"source_message_ids":[]})).unwrap();
    assert_eq!(patch.source_message_ids, Some(vec![]));
    let absent: crate::entities_v2::trace::PatchTraceDto =
        serde_json::from_value(serde_json::json!({})).unwrap();
    assert_eq!(absent.source_message_ids, None);
}

#[tokio::test]
#[ignore = "Requires explicitly configured disposable blog_test; synthetic data only; no notifications or LLM calls"]
async fn conversations_migration_privacy_membership_concurrency_and_trace_sources() {
    let url = std::env::var("BLOG_TEST_DATABASE_URL").unwrap();
    assert_eq!(url::Url::parse(&url).unwrap().path(), "/blog_test");
    let pool = DbPool::builder()
        .max_size(16)
        .build(ConnectionManager::<PgConnection>::new(url))
        .unwrap();
    const MIGRATIONS: diesel_migrations::EmbeddedMigrations = embed_migrations!();
    pool.get()
        .unwrap()
        .run_pending_migrations(MIGRATIONS)
        .unwrap();
    // Exercise an actual upgrade from legacy direct messages, not just empty tables.
    pool.get()
        .unwrap()
        .revert_last_migration(MIGRATIONS)
        .unwrap();
    let (a, b, c, stranger, mentor, journal) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    pool.get().unwrap().batch_execute(&format!("INSERT INTO users(id,email,first_name,last_name,handle,password,updated_at,ai_features_enabled,ai_features_enabled_by_admin)
        SELECT id,id::text||'@test.invalid','Synthetic','Person',id::text,'unused',NOW(),FALSE,FALSE
        FROM unnest(ARRAY['{a}'::uuid,'{b}','{c}','{stranger}','{mentor}']) id;
        UPDATE users SET principal_type='SERVICE',is_platform_user=FALSE WHERE id='{mentor}';
        INSERT INTO user_roles(user_id,role) VALUES('{mentor}','MENTOR');
        INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status)
        VALUES('{journal}','{b}','Synthetic','','','USER_JOURNAL','ACTIVE');
        INSERT INTO relationships(requester_user_id,target_user_id,relationship_type,status,accepted_at)
        VALUES('{a}','{b}','FOLLOW','ACCEPTED',NOW()),('{a}','{c}','FOLLOW','ACCEPTED',NOW()),('{b}','{c}','FOLLOW','ACCEPTED',NOW());
        INSERT INTO messages(sender_user_id,recipient_user_id,message_type,processing_state,title,content,created_at,seen_at)
        VALUES('{a}','{b}','GENERAL','PROCESSED','','Synthetic old one','2026-01-01 10:00',NOW()),
              ('{a}','{b}','GENERAL','PROCESSED','','Synthetic old two','2026-01-01 11:00',NULL),
              ('{a}','{b}','GENERAL','PROCESSED','','Synthetic old three','2026-01-01 12:00',NOW());")).unwrap();
    pool.get()
        .unwrap()
        .run_pending_migrations(MIGRATIONS)
        .unwrap();
    let direct_conv = service::create(
        a,
        CreateConversation {
            kind: ConversationKind::Direct,
            title: None,
            member_user_ids: vec![b],
        },
        &pool,
    )
    .unwrap();
    assert_eq!(
        service::get(b, direct_conv.id, &pool).unwrap().unread_count,
        1
    );
    assert_eq!(
        service::get(b, direct_conv.id, &pool)
            .unwrap()
            .membership
            .last_read_position,
        1
    );
    assert_eq!(
        service::create(
            b,
            CreateConversation {
                kind: ConversationKind::Direct,
                title: None,
                member_user_ids: vec![a]
            },
            &pool
        )
        .unwrap()
        .id,
        direct_conv.id
    );
    let old = service::messages(b, direct_conv.id, page(), None, &pool).unwrap();
    assert_eq!(
        old.items.iter().map(|m| m.position).collect::<Vec<_>>(),
        vec![3, 2, 1]
    );
    service::read(b, direct_conv.id, 3, &pool).unwrap();
    assert_eq!(
        service::get(b, direct_conv.id, &pool).unwrap().unread_count,
        0
    );
    let dm = direct(a, b, "Synthetic new direct", &pool);
    assert_eq!(dm.position, 4);
    assert_eq!(dm.conversation_id, direct_conv.id);
    assert_eq!(dm.recipient_user_id, Some(b));
    let mut pending = direct(a, b, "Synthetic pending placeholder", &pool);
    pending.processing_state = crate::entities_v2::message::MessageProcessingState::Pending;
    let mut pending = pending.update(&pool).unwrap();
    let later = direct(a, b, "Synthetic later processed message", &pool);
    let cursor = service::read(b, direct_conv.id, later.position, &pool).unwrap();
    assert_eq!(cursor.last_read_position, pending.position - 1);
    pending.processing_state = crate::entities_v2::message::MessageProcessingState::Processed;
    pending.content = "Synthetic completed reply".into();
    pending.update(&pool).unwrap();
    assert_eq!(
        service::get(b, direct_conv.id, &pool).unwrap().unread_count,
        1
    );
    let mentor_message = direct(mentor, b, "Synthetic mentor feedback", &pool);
    assert_eq!(
        service::get(b, mentor_message.conversation_id, &pool)
            .unwrap()
            .kind,
        ConversationKind::Direct
    );
    assert!(service::create(
        a,
        CreateConversation {
            kind: ConversationKind::Group,
            title: None,
            member_user_ids: vec![mentor]
        },
        &pool
    )
    .is_err());

    let g = group(a, vec![b], &pool);
    let first = send(a, g.id, "Synthetic before joining", &pool);
    assert_eq!(first.recipient_user_id, None);
    assert_eq!(first.position, 1);
    assert!(service::get(stranger, g.id, &pool).is_err());
    assert!(service::add_members(b, g.id, vec![c], &pool).is_err());
    service::add_members(a, g.id, vec![c], &pool).unwrap();
    assert_eq!(
        service::messages(c, g.id, page(), None, &pool)
            .unwrap()
            .total,
        0
    );
    assert!(crate::entities_v2::message::get_message_route(
        Extension(pool.clone()),
        Extension(session(c)),
        Path(first.id)
    )
    .await
    .is_err());
    let second = send(b, g.id, "Synthetic shared message", &pool);
    assert_eq!(
        service::messages(c, g.id, page(), None, &pool)
            .unwrap()
            .items[0]
            .id,
        second.id
    );
    assert_eq!(service::get(c, g.id, &pool).unwrap().unread_count, 1);
    assert!(service::notification_recipients(&second, &pool)
        .unwrap()
        .contains(&c));
    service::patch(
        c,
        g.id,
        PatchConversation {
            title: None,
            muted: Some(true),
        },
        &pool,
    )
    .unwrap();
    assert!(!service::notification_recipients(&second, &pool)
        .unwrap()
        .contains(&c));
    service::patch(
        c,
        g.id,
        PatchConversation {
            title: None,
            muted: Some(false),
        },
        &pool,
    )
    .unwrap();
    MessageReaction::upsert(second.id, c, "❤️", &pool).unwrap();
    let other = group(a, vec![b], &pool);
    let foreign = send(a, other.id, "Synthetic another group", &pool);
    assert!(service::send(
        b,
        g.id,
        serde_json::from_value(
            serde_json::json!({"content":"Wrong reply","reply_to_message_id":foreign.id})
        )
        .unwrap(),
        &pool
    )
    .is_err());
    // Blocking suppresses contact/pushes but doesn't hide an existing shared group.
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "INSERT INTO user_blocks(blocker_user_id,blocked_user_id) VALUES('{b}','{a}')"
        ))
        .unwrap();
    service::ensure_message_readable(first.id, b, &pool).unwrap();
    assert!(service::ensure_message_readable(dm.id, b, &pool).is_err());
    assert!(!service::notification_recipients(&first, &pool)
        .unwrap()
        .contains(&b));
    assert!(!service::reaction_notification_allowed(&first, b, a, &pool).unwrap());
    let cross = group(a, vec![c], &pool);
    service::change_member(a, cross.id, c, Some(MemberRole::Admin), &pool).unwrap();
    assert!(service::add_members(c, cross.id, vec![b], &pool).is_err());
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "DELETE FROM user_blocks WHERE blocker_user_id='{b}' AND blocked_user_id='{a}'"
        ))
        .unwrap();

    let mut new = NewTrace::new(
        "Synthetic copied trace".into(),
        "".into(),
        "Author-supplied text, not a server transcription".into(),
        Utc::now().naive_utc(),
        b,
        journal,
    );
    new.source_message_ids = vec![first.id, second.id];
    let copied = new.create(&pool).unwrap();
    assert_eq!(copied.source_message_ids.as_ref().unwrap().len(), 2);
    assert_eq!(
        copied.content,
        "Author-supplied text, not a server transcription"
    );
    let mut forbidden_source = NewTrace::new(
        "Synthetic".into(),
        "".into(),
        "Do not save".into(),
        Utc::now().naive_utc(),
        stranger,
        journal,
    );
    forbidden_source.source_message_ids = vec![first.id];
    assert!(forbidden_source.create(&pool).is_err());
    service::change_member(a, g.id, b, None, &pool).unwrap();
    assert!(service::get(b, g.id, &pool).is_err());
    assert!(service::ensure_message_readable(second.id, b, &pool).is_err()); // even own messages.
    assert!(MessageReaction::upsert(second.id, b, "👍", &pool).is_err());
    assert!(service::send(
        b,
        g.id,
        serde_json::from_value(serde_json::json!({"content":"No longer a member"})).unwrap(),
        &pool
    )
    .is_err());
    let mut edited = copied.clone();
    edited.content = "Still editable after leaving".into();
    let edited = edited
        .update_with_expected_version(copied.version_integer, &pool)
        .unwrap();
    assert_eq!(edited.source_message_ids.as_ref().unwrap().len(), 2);
    assert!(trace_sources::for_owner(&[edited.id], stranger, &pool)
        .unwrap()
        .is_empty());
    let read = crate::entities_v2::trace::get_trace_route(
        Extension(pool.clone()),
        Extension(session(b)),
        Path(edited.id),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(read.source_message_ids.as_ref().unwrap().len(), 2);
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "INSERT INTO trace_mentions(trace_id,mentioned_user_id,allows_reshare)
        VALUES('{}','{a}',FALSE)",
            edited.id
        ))
        .unwrap();
    assert!(crate::entities_v2::trace::get_trace_route(
        Extension(pool.clone()),
        Extension(session(a)),
        Path(edited.id)
    )
    .await
    .is_err());
    let edited = edited
        .set_status(crate::entities_v2::trace::TraceStatus::Finalized, &pool)
        .unwrap();
    let reader = crate::entities_v2::trace::get_trace_route(
        Extension(pool.clone()),
        Extension(session(a)),
        Path(edited.id),
    )
    .await
    .unwrap()
    .0;
    assert!(reader.source_message_ids.is_none());
    assert!(!serde_json::to_value(reader)
        .unwrap()
        .as_object()
        .unwrap()
        .contains_key("source_message_ids"));
    service::add_members(a, g.id, vec![b], &pool).unwrap();
    assert!(service::ensure_message_readable(second.id, b, &pool).is_err());
    let after = send(c, g.id, "Synthetic after rejoining", &pool);
    assert!(service::ensure_message_readable(after.id, b, &pool).is_ok());
    assert_eq!(
        service::messages(b, g.id, page(), None, &pool)
            .unwrap()
            .items
            .len(),
        1
    );
    // New source associations fail atomically, without replacing the text.
    let mut bad = edited.clone();
    bad.content = "Should roll back".into();
    bad.source_message_ids = Some(vec![foreign.id, Uuid::new_v4()]);
    assert!(bad
        .update_with_expected_version(edited.version_integer, &pool)
        .is_err());
    assert_eq!(
        Trace::find_full_trace(edited.id, &pool).unwrap().content,
        edited.content
    );

    let positions = std::thread::scope(|scope| {
        (0..8)
            .map(|_| scope.spawn(|| send(a, g.id, "Synthetic concurrent", &pool).position))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|t| t.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        positions
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        8
    );
    let high = *positions.iter().max().unwrap();
    std::thread::scope(|scope| {
        let one = scope.spawn(|| service::read(c, g.id, high, &pool).unwrap());
        let two = scope.spawn(|| service::read(c, g.id, second.position, &pool).unwrap());
        one.join().unwrap();
        two.join().unwrap();
    });
    assert_eq!(
        service::get(c, g.id, &pool)
            .unwrap()
            .membership
            .last_read_position,
        high
    );
    assert!(service::change_member(a, g.id, a, None, &pool).is_err());
    service::change_member(a, g.id, b, Some(MemberRole::Admin), &pool).unwrap();
    service::change_member(a, g.id, a, None, &pool).unwrap();
    assert!(service::get(a, g.id, &pool).is_err());
    assert!(service::list(b, page(), &pool).unwrap().items.len() >= 3); // repeated participant previews across groups.
    pool.get()
        .unwrap()
        .batch_execute(&format!("DELETE FROM messages WHERE id='{}'", first.id))
        .unwrap();
    assert!(Trace::find_full_trace(edited.id, &pool).is_ok());
    let orphan = group(a, vec![b], &pool);
    pool.get()
        .unwrap()
        .batch_execute(&format!("DELETE FROM users WHERE id='{a}'"))
        .unwrap();
    assert_eq!(
        service::get(b, orphan.id, &pool).unwrap().membership.role,
        MemberRole::Admin
    );
    // This database is disposable; cleanup is restricted to our synthetic users/groups.
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "DELETE FROM users WHERE id IN('{a}','{b}','{c}','{stranger}','{mentor}');
             DELETE FROM conversations WHERE id IN('{}','{}','{}','{}')",
            g.id, other.id, cross.id, orphan.id
        ))
        .unwrap();
}
