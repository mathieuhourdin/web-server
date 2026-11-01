use super::*;
use crate::entities_v2::{
    post::{NewPost, PostInteractionType, PostStatus, PostType},
    user_post_state::UserPostState,
};
use diesel::{connection::SimpleConnection, r2d2::ConnectionManager};

#[tokio::test]
#[ignore = "Requires an explicitly configured disposable blog_test database"]
async fn finalization_archives_content_once_including_direct_imports() {
    use crate::entities_v2::trace::{NewTrace, TraceStatus};
    let url = std::env::var("BLOG_TEST_DATABASE_URL").unwrap();
    assert_eq!(url::Url::parse(&url).unwrap().path(), "/blog_test");
    let pool = DbPool::builder().max_size(5).build(ConnectionManager::new(url)).unwrap();
    let owner = Uuid::new_v4();
    let journal = Uuid::new_v4();
    pool.get().unwrap().batch_execute(&format!("INSERT INTO users(id,email,first_name,last_name,handle,password,updated_at)
        VALUES('{owner}','{owner}@test.invalid','Author','Test','@{owner}','unused',NOW());
        INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status)
        VALUES('{journal}','{owner}','Test','','','USER_JOURNAL','ACTIVE');")).unwrap();
    let make = || NewTrace::new("A walk".into(), "".into(),
        "Today I went for a long walk through the forest and enjoyed the peaceful morning.".into(),
        chrono::Utc::now().naive_utc(), owner, journal);
    let draft = make().create(&pool).unwrap();
    let versions = |id| diesel::sql_query("SELECT * FROM trace_versions WHERE trace_id=$1 ORDER BY created_at,id")
        .bind::<SqlUuid,_>(id).load::<TraceVersion>(&mut pool.get().unwrap()).unwrap();
    assert!(versions(draft.id).is_empty());
    let mut trace = draft.set_status(TraceStatus::Finalized, &pool).unwrap();
    let snapshot = versions(trace.id).remove(0);
    assert_eq!(snapshot.content, trace.content);
    assert_eq!(snapshot.source_version, trace.version_integer);
    assert_eq!(snapshot.finalized_at, trace.finalized_at);
    assert_eq!(snapshot.language_code, trace.language_code);
    trace.content = "A later edit".into();
    let edited = trace.update(&pool).unwrap().set_status(TraceStatus::Finalized, &pool).unwrap();
    assert_eq!(versions(edited.id).len(), 1);
    assert_eq!(versions(edited.id)[0].content, snapshot.content);
    let direct = make().create_finalized(&pool).unwrap();
    assert_eq!(versions(direct.id).len(), 1);
    assert_eq!(versions(direct.id)[0].language_code, direct.language_code);
    assert!(direct.language_code.is_some());
    let historical_date = chrono::NaiveDate::from_ymd_opt(2020,1,2).unwrap().and_hms_opt(12,0,0).unwrap();
    let imported = make().create_imported(historical_date, &pool).unwrap();
    assert_eq!(versions(imported.id)[0].finalized_at, Some(historical_date));
    assert_eq!(versions(imported.id)[0].language_code, imported.language_code);
    let mut encrypted = make();
    encrypted.is_encrypted = true;
    encrypted.encryption_metadata = Some(serde_json::json!({"synthetic":true}));
    let encrypted = encrypted.create_finalized(&pool).unwrap();
    assert!(versions(encrypted.id).is_empty());
    pool.get().unwrap().batch_execute(&format!("DELETE FROM users WHERE id='{owner}'")).unwrap();
}

#[tokio::test]
#[ignore = "Requires an explicitly configured disposable blog_test database"]
async fn distribution_events_are_idempotent_archival_and_do_not_change_access_or_seen_state() {
    let url = std::env::var("BLOG_TEST_DATABASE_URL")
        .expect("Set BLOG_TEST_DATABASE_URL to disposable blog_test");
    assert_eq!(url::Url::parse(&url).unwrap().path(), "/blog_test");
    let pool = DbPool::builder()
        .max_size(5)
        .build(ConnectionManager::new(url))
        .unwrap();
    let owner = Uuid::new_v4();
    let reader = Uuid::new_v4();
    let journal = Uuid::new_v4();
    let trace = Uuid::new_v4();
    pool.get().unwrap().batch_execute(&format!("INSERT INTO users(id,email,first_name,last_name,handle,password,updated_at)
        VALUES('{owner}','{owner}@test.invalid','Author','Test','@{owner}','unused',NOW()),
              ('{reader}','{reader}@test.invalid','Reader','Test','@{reader}','unused',NOW());
        INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status) VALUES('{journal}','{owner}','Test','','','USER_JOURNAL','ACTIVE');
        INSERT INTO relationships(requester_user_id,target_user_id,relationship_type,status,accepted_at) VALUES('{reader}','{owner}','FOLLOW','ACCEPTED',NOW());
        INSERT INTO traces(id,user_id,journal_id,title,subtitle,content,interaction_date,trace_type,status,start_writing_at,finalized_at)
        VALUES('{trace}','{owner}','{journal}','Test','','Original',NOW(),'USER_TRACE','FINALIZED',NOW(),NOW());")).unwrap();
    let mut post = NewPost {
        source_trace_id: Some(trace),
        source_document_id: None,
        source_album_id: None,
        post_type: PostType::Idea,
        interaction_type: PostInteractionType::Output,
        user_id: owner,
        publishing_date: None,
        status: PostStatus::Draft,
    }
    .create(&pool)
    .unwrap();
    assert!(post.last_distribution_event_id.is_none());
    let postid = post.id;
    pool.get().unwrap().batch_execute(&format!("INSERT INTO post_grants(id,post_id,owner_user_id,grantee_user_id,access_level,status) VALUES(gen_random_uuid(),'{postid}','{owner}','{reader}','READ','ACTIVE');")).unwrap();
    post.notification_mode = PostNotificationMode::None;
    post.status = PostStatus::Published;
    post.publishing_date = Some(chrono::Utc::now().naive_utc() - chrono::Duration::days(2));
    let post = post.update(&pool).unwrap();
    let first = post.last_distribution_event_id.unwrap();
    let readable = crate::entities_v2::trace::Trace::find_full_trace(trace, &pool).unwrap();
    assert!(readable.user_can_read(reader, &pool).unwrap());
    assert_eq!(
        Post::find_readable_for_trace(trace, reader, &pool)
            .unwrap()
            .unwrap()
            .id,
        postid
    );
    assert_eq!(
        crate::entities_v2::feed::hydrate::count_recent_unread_feed_items(
            reader,
            chrono::Utc::now().naive_utc() - chrono::Duration::days(3),
            &pool
        )
        .unwrap(),
        1
    );
    let image = Uuid::new_v4();
    pool.get().unwrap().batch_execute(&format!("INSERT INTO assets(id,owner_user_id,bucket,object_key,mime_type,original_filename,size_bytes,status)
        VALUES('{image}','{owner}','synthetic-test-only','{image}.jpg','image/jpeg','test.jpg',1,'READY');
        UPDATE traces SET content_image_asset_id='{image}' WHERE id='{trace}';
        INSERT INTO trace_plumes(trace_id,user_id) VALUES('{trace}','{reader}');")).unwrap();
    assert!(crate::entities_v2::asset::Asset::can_user_read(image, reader, &pool).unwrap());
    let mut reader_session = Session::anonymous();
    reader_session.user_id = Some(reader);
    // An existing plume exercises the real PUT path without scheduling a push.
    let plume = crate::entities_v2::trace::plume::put(
        Extension(pool.clone()),
        Extension(reader_session.clone()),
        Path(trace),
    )
    .await
    .unwrap()
    .0;
    assert!(plume.plumed_by_me);
    assert_eq!(plume.plume_count, 1);
    UserPostState::create_or_mark_seen(reader, postid, &pool).unwrap();
    let mut session = Session::anonymous();
    session.user_id = Some(owner);
    let version = archive_trace(
        Extension(pool.clone()),
        Extension(session.clone()),
        Path(trace),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(version.content, "Original");
    assert!(pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE trace_versions SET content='Tampered' WHERE id='{}'",
            version.id
        ))
        .is_err());
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET content='Live edit' WHERE id='{trace}'"
        ))
        .unwrap();
    assert_eq!(
        Post::find_full(postid, &pool)
            .unwrap()
            .last_distribution_event_id,
        Some(first)
    );
    let key = Uuid::new_v4();
    let input = || {
        serde_json::from_value::<PostDistributionEventInput>(serde_json::json!({
            "idempotency_key":key,"trace_version_id":version.id,
            "notification_mode":"selected","notification_user_ids":[reader]
        }))
        .unwrap()
    };
    let (event, created) = create(postid, owner, input(), &pool).unwrap();
    assert!(created);
    let (repeated, created) = create(postid, owner, input(), &pool).unwrap();
    assert!(!created);
    assert_eq!(event.id, repeated.id);
    assert_eq!(event.trace_version_id, Some(version.id));
    let event_json = serde_json::to_value(&event).unwrap();
    assert!(event_json["distributed_at"].is_string());
    assert!(event_json.get("published_at").is_none());
    let post = Post::find_full(postid, &pool).unwrap();
    assert_eq!(post.last_distribution_event_id, Some(event.id));
    let post_json = serde_json::to_value(&post).unwrap();
    assert!(post_json["last_distribution_event_id"].is_string());
    assert!(post_json.get("last_publication_id").is_none());
    assert!(post_json.get("last_publication_at").is_none());
    assert_eq!(eligible_recipients(&post, &pool).unwrap(), vec![reader]);
    let feed =
        crate::entities_v2::feed::hydrate::find_feed_items_paginated(reader, None, 0, 20, &pool)
            .unwrap()
            .0;
    assert_eq!(feed[0].last_distribution_event_id, Some(event.id));
    assert_eq!(feed[0].feed_date, event.distributed_at);
    assert_eq!(feed[0].content, "Live edit");
    assert!(claim(event.id, &pool).unwrap());
    assert!(!claim(event.id, &pool).unwrap());
    assert!(claim_push(event.id, reader, &pool).unwrap());
    assert!(!claim_push(event.id, reader, &pool).unwrap());
    finish_push(event.id, reader, true, &pool).unwrap();
    assert!(
        crate::entities_v2::notification::successfully_pushed_post_ids(reader, &[postid], &pool)
            .unwrap()
            .contains(&postid)
    );
    #[derive(QueryableByName)]
    struct Seen {
        #[diesel(sql_type=BigInt)]
        count: i64,
    }
    assert_eq!(
        diesel::sql_query("SELECT COUNT(*) FROM user_post_states WHERE post_id=$1 AND user_id=$2")
            .bind::<SqlUuid, _>(postid)
            .bind::<SqlUuid, _>(reader)
            .get_result::<Seen>(&mut pool.get().unwrap())
            .unwrap()
            .count,
        1
    );
    let mut stranger = Session::anonymous();
    stranger.user_id = Some(reader);
    assert!(get_trace_version(
        Extension(pool.clone()),
        Extension(stranger),
        Path((trace, version.id))
    )
    .await
    .is_err());
    assert!(pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE post_distribution_events SET distributed_at=NOW() WHERE id='{}'",
            event.id
        ))
        .is_err());
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE post_grants SET status='REVOKED' WHERE post_id='{postid}'"
        ))
        .unwrap();
    assert!(eligible_recipients(&post, &pool).unwrap().is_empty());
    assert!(!readable.user_can_read(reader, &pool).unwrap());
    assert!(!crate::entities_v2::asset::Asset::can_user_read(image, reader, &pool).unwrap());
    assert!(crate::entities_v2::trace::plume::put(
        Extension(pool.clone()),
        Extension(reader_session),
        Path(trace)
    )
    .await
    .is_err());
    let bad = PostDistributionEventInput {
        idempotency_key: Uuid::new_v4(),
        ..input()
    };
    assert!(create(postid, owner, bad, &pool).is_err());
    let versions = get_trace_versions(
        Extension(pool.clone()),
        Extension(session),
        Path(trace),
        Query(PaginationParams::from_parts(None, None)),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(versions.items[0].content, "Original");
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE posts SET status='ARCHIVED' WHERE id='{postid}';
             DELETE FROM users WHERE id IN('{owner}','{reader}')"
        ))
        .unwrap();
}
