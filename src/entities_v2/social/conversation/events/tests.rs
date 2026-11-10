use super::*;
use std::time::Duration;

#[test]
fn subscriptions_are_bounded_and_removed_on_drop() {
    let hub = Arc::new(Hub::new());
    let user = Uuid::new_v4();
    let mut subscriptions = (0..MAX_STREAMS_PER_USER)
        .map(|_| hub.subscribe(user).unwrap())
        .collect::<Vec<_>>();
    assert!(hub.subscribe(user).is_none());
    for _ in 0..CAPACITY + 1 {
        hub.resync("test_gap");
    }
    assert!(matches!(
        subscriptions[0].receiver.try_recv(),
        Err(broadcast::error::TryRecvError::Lagged(_))
    ));
    drop(subscriptions);
    assert!(hub.users().is_empty());
    assert_eq!(hub.subscribers.lock().unwrap().total, 0);
}

#[tokio::test]
#[ignore = "Requires explicitly configured disposable blog_test; synthetic content only, no real notifications/LLMs"]
async fn postgres_events_are_committed_authorized_and_resynchronized() {
    use crate::{
        db::DbPool,
        entities_v2::social::conversation::{
            model::{ConversationKind, CreateConversation},
            service,
        },
        entities_v2::{
            message::{MessageProcessingState, MessageReaction, NewMessage, NewMessageDto},
            session::Session,
        },
    };
    use diesel::{
        connection::SimpleConnection,
        prelude::*,
        r2d2::ConnectionManager,
        sql_types::{Array, Integer, Text, Uuid as SqlUuid},
    };
    use diesel_migrations::{embed_migrations, MigrationHarness};
    use futures_util::StreamExt;
    use routes::{project, Projection};
    use tokio::time::{sleep, timeout};
    let url = std::env::var("BLOG_TEST_DATABASE_URL").unwrap();
    let parsed = url::Url::parse(&url).unwrap();
    assert_eq!(parsed.path(), "/blog_test");
    assert_eq!(parsed.host_str(), Some("127.0.0.1"));
    assert_eq!(parsed.port(), Some(55439));
    let pool = DbPool::builder()
        .max_size(8)
        .build(ConnectionManager::<diesel::PgConnection>::new(url.clone()))
        .unwrap();
    const MIGRATIONS: diesel_migrations::EmbeddedMigrations = embed_migrations!();
    pool.get()
        .unwrap()
        .run_pending_migrations(MIGRATIONS)
        .unwrap();
    let (a, b, c, mentor) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    struct Cleanup(DbPool, Vec<Uuid>);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            diesel::sql_query("DELETE FROM sessions WHERE user_id=ANY($1)")
                .bind::<Array<SqlUuid>, _>(&self.1)
                .execute(&mut *self.0.get().unwrap())
                .unwrap();
            diesel::sql_query("DELETE FROM users WHERE id=ANY($1)")
                .bind::<Array<SqlUuid>, _>(&self.1)
                .execute(&mut *self.0.get().unwrap())
                .unwrap();
        }
    }
    let _cleanup = Cleanup(pool.clone(), vec![a, b, c, mentor]);
    pool.get().unwrap().batch_execute(&format!("INSERT INTO users(id,email,first_name,last_name,handle,password)
        SELECT id,id::text||'@test.invalid','Synthetic','SSE test',id::text,'unused' FROM unnest(ARRAY['{a}'::uuid,'{b}','{c}','{mentor}']) id;
        UPDATE users SET principal_type='SERVICE',is_platform_user=FALSE WHERE id='{mentor}';
        INSERT INTO user_roles(user_id,role) VALUES('{mentor}','MENTOR');
        INSERT INTO relationships(requester_user_id,target_user_id,relationship_type,status,accepted_at)
        VALUES('{a}','{b}','FOLLOW','ACCEPTED',NOW()),('{a}','{c}','FOLLOW','ACCEPTED',NOW());")).unwrap();
    let group = service::create(
        a,
        CreateConversation {
            kind: ConversationKind::Group,
            title: None,
            member_user_ids: vec![b],
        },
        &pool,
    )
    .unwrap()
    .conversation
    .id;
    let (session_b, _) = Session::create_authenticated(b, &pool).unwrap();
    let (session_c, _) = Session::create_authenticated(c, &pool).unwrap();
    let hub = Arc::new(Hub::new());
    let mut sub_b = hub.subscribe(b).unwrap();
    let mut sub_c = hub.subscribe(c).unwrap();
    struct Listener(tokio::task::JoinHandle<()>);
    impl Drop for Listener {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let _listener = Listener(tokio::spawn(listener::run(url, hub.clone())));
    timeout(Duration::from_secs(10), async {
        while !hub.ready.load(Ordering::SeqCst) {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    while sub_b.receiver.try_recv().is_ok() {}
    while sub_c.receiver.try_recv().is_ok() {}
    // Verify the actual SSE body contract and that dropping it frees its slot.
    let response = routes::stream_with_hub(pool.clone(), session_b.clone(), hub.clone())
        .await
        .unwrap();
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    assert_eq!(response.headers()["x-accel-buffering"], "no");
    let mut body = response.into_body().into_data_stream();
    let initial = timeout(Duration::from_secs(2), body.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(std::str::from_utf8(&initial)
        .unwrap()
        .contains("event: resync_required"));
    drop(body);

    let rolled_back=pool.get().unwrap().transaction::<(),diesel::result::Error,_>(|conn| {
        diesel::sql_query("INSERT INTO messages(sender_user_id,conversation_id,message_type,processing_state,title,content)
            VALUES($1,$2,'GENERAL','PROCESSED','','synthetic rollback')")
            .bind::<SqlUuid,_>(a).bind::<SqlUuid,_>(group).execute(conn)?;
        Err(diesel::result::Error::RollbackTransaction)
    });
    assert!(rolled_back.is_err());
    assert!(timeout(Duration::from_millis(100), sub_b.receiver.recv())
        .await
        .is_err());
    // Any active member may set another member's shared contextual nickname.
    service::patch_member(
        b,
        group,
        a,
        serde_json::from_value(serde_json::json!({"nickname":"  Captain  "})).unwrap(),
        &pool,
    )
    .unwrap();
    let renamed=timeout(Duration::from_secs(5),async {
        loop {if let Wake::Change(notice)=sub_b.receiver.recv().await.unwrap(){if matches!(&*notice,Notice::MembershipChanged{conversation_id,user_id} if *conversation_id==group && *user_id==a){return notice;}}}
    }).await.unwrap();
    assert!(!format!("{renamed:?}").contains("Captain"));
    let Projection::Event(data) = project(b, session_b.id, &renamed, &pool).unwrap() else {
        panic!("nickname event expected")
    };
    assert_eq!(data.name, "conversation_updated");
    let view = service::get(a, group, &pool).unwrap();
    assert_eq!(view.membership.nickname.as_deref(), Some("Captain"));
    let member = view
        .members_preview
        .iter()
        .find(|member| member.user.id == a)
        .unwrap();
    assert_eq!(member.nickname.as_deref(), Some("Captain"));
    assert_ne!(member.user.display_name, "Captain");
    assert!(service::patch_member(
        c,
        group,
        b,
        serde_json::from_value(serde_json::json!({"nickname":"outsider"})).unwrap(),
        &pool
    )
    .is_err());
    assert!(service::patch_member(
        b,
        group,
        a,
        serde_json::from_value(serde_json::json!({"role":"member","nickname":"not saved"}))
            .unwrap(),
        &pool
    )
    .is_err());
    assert_eq!(
        service::get(a, group, &pool)
            .unwrap()
            .membership
            .nickname
            .as_deref(),
        Some("Captain")
    );
    service::patch_member(
        b,
        group,
        a,
        serde_json::from_value(serde_json::json!({"nickname":null})).unwrap(),
        &pool,
    )
    .unwrap();
    assert!(service::get(a, group, &pool)
        .unwrap()
        .membership
        .nickname
        .is_none());
    let message = service::send(
        a,
        group,
        serde_json::from_value(serde_json::json!({"content":"SYNTHETIC private message"})).unwrap(),
        &pool,
    )
    .unwrap();
    async fn next_message(sub: &mut Subscription, id: Uuid) -> Arc<Notice> {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Wake::Change(notice) = sub.receiver.recv().await.unwrap() {
                    if notice.message_ref().is_some_and(|(mid, _)| mid == id) {
                        return notice;
                    }
                }
            }
        })
        .await
        .unwrap()
    }
    let created = next_message(&mut sub_b, message.id).await;
    assert!(matches!(&*created, Notice::MessageCreated { .. }));
    assert!(!format!("{created:?}").contains("SYNTHETIC"));
    assert!(timeout(Duration::from_millis(100), sub_c.receiver.recv())
        .await
        .is_err());
    let Projection::Event(data) = project(b, session_b.id, &created, &pool).unwrap() else {
        panic!("visible message expected")
    };
    assert_eq!(data.name, "message_created");
    assert_eq!(data.data["message"]["content"], message.content);
    assert!(matches!(
        project(c, session_c.id, &created, &pool).unwrap(),
        Projection::Skip
    ));
    MessageReaction::upsert(message.id, b, "❤️", &pool).unwrap();
    let reaction = next_message(&mut sub_b, message.id).await;
    let Projection::Event(data) = project(b, session_b.id, &reaction, &pool).unwrap() else {
        panic!("visible reaction expected")
    };
    assert_eq!(data.name, "reaction_updated");
    assert_eq!(data.data["message"]["reactions"][0]["reacted_by_me"], true);
    // Read acknowledgements update only the member's summary, not message text.
    diesel::sql_query("UPDATE messages SET seen_at=NOW() WHERE id=$1")
        .bind::<SqlUuid, _>(message.id)
        .execute(&mut *pool.get().unwrap())
        .unwrap();
    assert!(timeout(Duration::from_millis(100), sub_b.receiver.recv())
        .await
        .is_err());
    service::add_members(a, group, vec![c], &pool).unwrap();
    assert!(matches!(
        project(c, session_c.id, &created, &pool).unwrap(),
        Projection::Skip
    ));
    // A worker-style pending mentor row becomes live only on completion.
    let input: NewMessageDto = serde_json::from_value(
        serde_json::json!({"recipient_user_id":b,"content":"","message_type":"mentor_reply"}),
    )
    .unwrap();
    let mut pending = NewMessage::new(input, mentor);
    pending.processing_state = MessageProcessingState::Pending;
    let pending = pending.create(&pool).unwrap();
    diesel::sql_query("UPDATE messages SET content='Synthetic mentor completion',processing_state='PROCESSED' WHERE id=$1")
        .bind::<SqlUuid,_>(pending.id).execute(&mut *pool.get().unwrap()).unwrap();
    let completed = next_message(&mut sub_b, pending.id).await;
    assert!(matches!(&*completed, Notice::MessageCreated { .. }));
    let Projection::Event(data) = project(b, session_b.id, &completed, &pool).unwrap() else {
        panic!("mentor reply expected")
    };
    assert_eq!(
        data.data["message"]["content"],
        "Synthetic mentor completion"
    );
    // An old deletion must not disclose an ID/position to a newly joined member.
    diesel::sql_query("DELETE FROM messages WHERE id=$1")
        .bind::<SqlUuid, _>(message.id)
        .execute(&mut *pool.get().unwrap())
        .unwrap();
    let deleted = next_message(&mut sub_b, message.id).await;
    assert!(matches!(&*deleted, Notice::MessageDeleted { .. }));
    assert!(matches!(
        project(c, session_c.id, &deleted, &pool).unwrap(),
        Projection::Skip
    ));
    service::patch_member(
        a,
        group,
        b,
        serde_json::from_value(serde_json::json!({"nickname":"Old admission"})).unwrap(),
        &pool,
    )
    .unwrap();
    service::change_member(a, group, b, None, &pool).unwrap();
    let removed = Notice::MembershipChanged {
        conversation_id: group,
        user_id: b,
    };
    let Projection::Event(data) = project(b, session_b.id, &removed, &pool).unwrap() else {
        panic!("removal expected")
    };
    assert_eq!(data.name, "conversation_removed");
    assert!(matches!(
        project(b, session_b.id, &deleted, &pool).unwrap(),
        Projection::Skip
    ));
    service::add_members(a, group, vec![b], &pool).unwrap();
    assert!(service::get(b, group, &pool)
        .unwrap()
        .membership
        .nickname
        .is_none());
    let direct = pending.conversation_id;
    service::patch_member(
        b,
        direct,
        mentor,
        serde_json::from_value(serde_json::json!({"nickname":"My mentor"})).unwrap(),
        &pool,
    )
    .unwrap();
    assert_eq!(
        service::get(b, direct, &pool)
            .unwrap()
            .members_preview
            .iter()
            .find(|m| m.user.id == mentor)
            .unwrap()
            .nickname
            .as_deref(),
        Some("My mentor")
    );

    // Terminate only the named listener in this disposable database. The hub
    // must signal a gap and reconnect, without retaining any event history.
    while sub_b.receiver.try_recv().is_ok() {}
    #[derive(QueryableByName)]
    struct Pid {
        #[diesel(sql_type=Integer)]
        pid: i32,
    }
    let pids = diesel::sql_query(
        "SELECT pid FROM pg_stat_activity WHERE datname=current_database() AND application_name=$1",
    )
    .bind::<Text, _>("hupo_conversation_events")
    .load::<Pid>(&mut *pool.get().unwrap())
    .unwrap();
    assert_eq!(pids.len(), 1);
    diesel::sql_query("SELECT pg_terminate_backend($1)")
        .bind::<Integer, _>(pids[0].pid)
        .execute(&mut *pool.get().unwrap())
        .unwrap();
    timeout(Duration::from_secs(10), async {
        let mut gap = false;
        loop {
            match sub_b.receiver.recv().await.unwrap() {
                Wake::Resync("listener_disconnected") => gap = true,
                Wake::Resync("listener_connected") if gap => break,
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    Session::revoke(session_b.id, &pool).unwrap();
    assert!(matches!(
        project(b, session_b.id, &completed, &pool).unwrap(),
        Projection::Unauthorized
    ));
}
