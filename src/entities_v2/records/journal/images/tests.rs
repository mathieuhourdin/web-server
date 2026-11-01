use super::*;
use diesel::{connection::SimpleConnection, r2d2::ConnectionManager};

#[test]
#[ignore = "Requires an explicitly configured disposable blog_test database"]
fn journal_images_are_paginated_and_respect_current_trace_access() {
    let url = std::env::var("BLOG_TEST_DATABASE_URL")
        .expect("Set BLOG_TEST_DATABASE_URL to disposable blog_test");
    assert_eq!(url::Url::parse(&url).unwrap().path(), "/blog_test");
    let pool = DbPool::builder()
        .max_size(4)
        .build(ConnectionManager::new(url))
        .unwrap();
    let owner = Uuid::new_v4();
    let reader = Uuid::new_v4();
    let outsider = Uuid::new_v4();
    let journal = Uuid::new_v4();
    let empty = Uuid::new_v4();
    let visible = Uuid::new_v4();
    let mentioned = Uuid::new_v4();
    let hidden = Uuid::new_v4();
    let asset = Uuid::new_v4();
    let failed_asset = Uuid::new_v4();
    let post = Uuid::new_v4();
    let channel = Uuid::new_v4();
    pool.get().unwrap().batch_execute(&format!("
        INSERT INTO users(id,email,first_name,last_name,handle,password,updated_at)
        VALUES('{owner}','{owner}@test.invalid','Owner','Test','@{owner}','unused',NOW()),
              ('{reader}','{reader}@test.invalid','Reader','Test','@{reader}','unused',NOW()),
              ('{outsider}','{outsider}@test.invalid','Other','Test','@{outsider}','unused',NOW());
        INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status)
        VALUES('{journal}','{owner}','Images','','','USER_JOURNAL','ACTIVE'),
              ('{empty}','{owner}','Empty','','','USER_JOURNAL','ACTIVE');
        INSERT INTO relationships(requester_user_id,target_user_id,relationship_type,status,accepted_at)
        VALUES('{reader}','{owner}','FOLLOW','ACCEPTED',NOW());
        INSERT INTO assets(id,owner_user_id,bucket,object_key,mime_type,original_filename,size_bytes,status,image_width,image_height)
        VALUES('{asset}','{owner}','test','{asset}','image/jpeg','test.jpg',100,'READY',1200,1600),
              ('{failed_asset}','{owner}','test','{failed_asset}','image/jpeg','test.jpg',100,'FAILED',NULL,NULL);
        INSERT INTO traces(id,user_id,journal_id,title,subtitle,content,interaction_date,trace_type,status,start_writing_at,finalized_at,content_image_asset_id)
        VALUES('{visible}','{owner}','{journal}','Visible','','',NOW(),'USER_TRACE','FINALIZED','2026-01-01','2026-01-01','{asset}'),
              ('{mentioned}','{owner}','{journal}','Mentioned','','',NOW(),'USER_TRACE','FINALIZED','2026-01-02','2026-01-02','{asset}'),
              ('{hidden}','{owner}','{journal}','Hidden','','',NOW(),'USER_TRACE','FINALIZED','2026-01-03','2026-01-03','{asset}'),
              (gen_random_uuid(),'{owner}','{journal}','Failed image','','',NOW(),'USER_TRACE','FINALIZED','2026-01-04','2026-01-04','{failed_asset}'),
              (gen_random_uuid(),'{owner}','{journal}','Archived','','',NOW(),'USER_TRACE','ARCHIVED','2026-01-05','2026-01-05','{asset}');
        INSERT INTO traces(user_id,journal_id,title,subtitle,content,interaction_date,trace_type,status,content_image_asset_id)
        VALUES('{owner}','{journal}','Draft','','',NOW(),'USER_TRACE','DRAFT','{asset}');
        INSERT INTO posts(id,user_id,source_trace_id,title,subtitle,content,interaction_type,post_type,status,notification_mode)
        VALUES('{post}','{owner}','{visible}','','','','OUTPUT','IDEA','PUBLISHED','NONE');
        INSERT INTO post_grants(id,post_id,owner_user_id,grantee_user_id,access_level,status)
        VALUES(gen_random_uuid(),'{post}','{owner}','{reader}','READ','ACTIVE');
        INSERT INTO trace_mentions(trace_id,mentioned_user_id) VALUES('{mentioned}','{reader}');
        INSERT INTO publication_channels(id,user_id,kind,enabled) VALUES('{channel}','{owner}','BLOG',true);
        INSERT INTO posts(user_id,publication_channel_id,source_trace_id,title,subtitle,content,interaction_type,post_type,status,notification_mode)
        VALUES('{owner}','{channel}','{hidden}','','','','OUTPUT','IDEA','PUBLISHED','NONE');
    ")).unwrap();
    let journal = Journal::find_full(journal, &pool).unwrap();
    let page = ValidatedPagination {
        offset: 0,
        limit: 1,
    };
    let owned = journal.images_paginated(owner, page, &pool).unwrap();
    assert_eq!(owned.total, 3);
    assert_eq!(owned.items.len(), 1);
    assert_eq!(owned.items[0].trace_id, hidden);
    let shared = journal.images_paginated(reader, page, &pool).unwrap();
    assert_eq!(shared.total, 2);
    assert_eq!(shared.items[0].trace_id, mentioned);
    let json = serde_json::to_value(&shared).unwrap();
    assert_eq!(json["items"][0]["image"]["id"], asset.to_string());
    assert_eq!(json["items"][0]["image"]["width"], 1200);
    assert_eq!(json["items"][0]["image"]["height"], 1600);
    assert_eq!(json["items"][0]["finalized_at"], "2026-01-02T00:00:00Z");
    let next = journal
        .images_paginated(
            reader,
            ValidatedPagination {
                offset: 1,
                limit: 1,
            },
            &pool,
        )
        .unwrap();
    assert_eq!(next.items[0].trace_id, visible);
    let beyond = journal
        .images_paginated(
            reader,
            ValidatedPagination {
                offset: 99,
                limit: 1,
            },
            &pool,
        )
        .unwrap();
    assert_eq!(beyond.total, 2);
    assert!(beyond.items.is_empty());
    assert!(journal.images_paginated(outsider, page, &pool).is_err());
    let empty_journal = Journal::find_full(empty, &pool).unwrap();
    assert_eq!(
        empty_journal
            .images_paginated(owner, page, &pool)
            .unwrap()
            .total,
        0
    );
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE post_grants SET status='REVOKED' WHERE post_id='{post}';"
        ))
        .unwrap();
    assert_eq!(
        journal.images_paginated(reader, page, &pool).unwrap().total,
        1
    );
    // The journal remains readable through the mention, but no ready image remains.
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET content_image_asset_id=NULL WHERE id='{mentioned}';"
        ))
        .unwrap();
    let no_images = journal.images_paginated(reader, page, &pool).unwrap();
    assert_eq!(no_images.total, 0);
    assert!(no_images.items.is_empty());
    pool.get()
        .unwrap()
        .batch_execute(&format!(
        "INSERT INTO user_blocks(blocker_user_id,blocked_user_id) VALUES('{owner}','{reader}');"
    ))
        .unwrap();
    assert!(journal.images_paginated(reader, page, &pool).is_err());
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "DELETE FROM users WHERE id IN('{owner}','{reader}','{outsider}');"
        ))
        .unwrap();
}
