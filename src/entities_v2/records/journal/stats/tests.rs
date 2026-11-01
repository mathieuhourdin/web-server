use super::*;
use diesel::{connection::SimpleConnection, r2d2::ConnectionManager};

#[test]
#[ignore = "Requires an explicitly configured disposable blog_test database"]
fn journal_stats_are_batched_and_only_count_visible_finalized_traces() {
    let url = std::env::var("BLOG_TEST_DATABASE_URL")
        .expect("Set BLOG_TEST_DATABASE_URL to disposable blog_test");
    assert_eq!(url::Url::parse(&url).unwrap().path(), "/blog_test");
    let pool = DbPool::builder()
        .max_size(4)
        .build(ConnectionManager::new(url))
        .unwrap();
    let owner = Uuid::new_v4();
    let reader = Uuid::new_v4();
    let journal = Uuid::new_v4();
    let empty = Uuid::new_v4();
    let visible = Uuid::new_v4();
    let hidden = Uuid::new_v4();
    let mentioned = Uuid::new_v4();
    let post = Uuid::new_v4();
    let channel = Uuid::new_v4();
    pool.get().unwrap().batch_execute(&format!("INSERT INTO users(id,email,first_name,last_name,handle,password,updated_at)
        VALUES('{owner}','{owner}@test.invalid','Owner','Test','@{owner}','unused',NOW()),
              ('{reader}','{reader}@test.invalid','Reader','Test','@{reader}','unused',NOW());
        INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status)
        VALUES('{journal}','{owner}','Test','','','USER_JOURNAL','ACTIVE'),('{empty}','{owner}','Empty','','','USER_JOURNAL','ACTIVE');
        INSERT INTO relationships(requester_user_id,target_user_id,relationship_type,status,accepted_at)
        VALUES('{reader}','{owner}','FOLLOW','ACCEPTED',NOW());
        INSERT INTO traces(id,user_id,journal_id,title,subtitle,content,interaction_date,trace_type,status,start_writing_at,finalized_at)
        VALUES('{visible}','{owner}','{journal}','Visible','','Content',NOW(),'USER_TRACE','FINALIZED','2026-01-02','2026-01-02'),
              ('{hidden}','{owner}','{journal}','Hidden','','Content',NOW(),'USER_TRACE','FINALIZED','2026-10-30','2026-10-30'),
              ('{mentioned}','{owner}','{journal}','Mentioned','','Content',NOW(),'USER_TRACE','FINALIZED','2026-02-03','2026-02-03'),
              (gen_random_uuid(),'{owner}','{journal}','Archived','','Content',NOW(),'USER_TRACE','ARCHIVED','2020-01-01','2020-01-01');
        INSERT INTO traces(user_id,journal_id,title,subtitle,content,interaction_date,trace_type,status)
        VALUES('{owner}','{journal}','Draft','','Content',NOW(),'USER_TRACE','DRAFT');
        INSERT INTO posts(id,user_id,source_trace_id,title,subtitle,content,interaction_type,post_type,status,notification_mode)
        VALUES('{post}','{owner}','{visible}','','','','OUTPUT','IDEA','PUBLISHED','NONE');
        INSERT INTO post_grants(id,post_id,owner_user_id,grantee_user_id,access_level,status)
        VALUES(gen_random_uuid(),'{post}','{owner}','{reader}','READ','ACTIVE');
        INSERT INTO trace_mentions(trace_id,mentioned_user_id) VALUES('{mentioned}','{reader}');
        INSERT INTO publication_channels(id,user_id,kind,enabled) VALUES('{channel}','{owner}','BLOG',true);
        INSERT INTO posts(user_id,publication_channel_id,source_trace_id,title,subtitle,content,interaction_type,post_type,status,notification_mode)
        VALUES('{owner}','{channel}','{hidden}','','','','OUTPUT','IDEA','PUBLISHED','NONE');
    ")).unwrap();
    let journals = vec![
        Journal::find_full(journal, &pool).unwrap(),
        Journal::find_full(empty, &pool).unwrap(),
    ];
    let owned = Journal::hydrate_stats(journals.clone(), owner, &pool).unwrap();
    let stats = owned[0].stats.as_ref().unwrap();
    assert_eq!(stats.traces_count, 3);
    assert_eq!(
        stats.oldest_trace_finalized_at.unwrap().to_rfc3339(),
        "2026-01-02T00:00:00+00:00"
    );
    assert_eq!(
        stats.newest_trace_finalized_at.unwrap().to_rfc3339(),
        "2026-10-30T00:00:00+00:00"
    );
    assert_eq!(owned[1].stats.as_ref().unwrap(), &JournalStats::default());
    let shared = Journal::hydrate_stats(journals.clone(), reader, &pool).unwrap();
    let stats = shared[0].stats.as_ref().unwrap();
    assert_eq!(stats.traces_count, 2);
    assert_eq!(
        stats.newest_trace_finalized_at.unwrap().to_rfc3339(),
        "2026-02-03T00:00:00+00:00"
    );
    let json = serde_json::to_value(&shared[0]).unwrap();
    assert_eq!(json["stats"]["traces_count"], 2);
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE post_grants SET status='REVOKED' WHERE post_id='{post}';"
        ))
        .unwrap();
    let shared = Journal::hydrate_stats(journals.clone(), reader, &pool).unwrap();
    assert_eq!(shared[0].stats.as_ref().unwrap().traces_count, 1);
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "INSERT INTO user_blocks(blocker_user_id,blocked_user_id) VALUES('{owner}','{reader}')"
        ))
        .unwrap();
    let blocked = Journal::hydrate_stats(journals, reader, &pool).unwrap();
    assert_eq!(blocked[0].stats.as_ref().unwrap(), &JournalStats::default());
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "DELETE FROM users WHERE id IN('{owner}','{reader}')"
        ))
        .unwrap();
}
