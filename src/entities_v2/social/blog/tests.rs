use super::model::*;
use crate::{
    db::DbPool,
    entities_v2::post::{NewPost, Post, PostInteractionType, PostStatus, PostType},
    pagination::ValidatedPagination,
};
use diesel::{connection::SimpleConnection, r2d2::ConnectionManager};
use uuid::Uuid;

#[test]
fn validates_blog_identity() {
    let mut input = BlogInput {
        slug: "notes-de-travail".into(),
        name: "Notes".into(),
        username: "Le Promeneur".into(),
        description: String::new(),
        image_asset_id: None,
        enabled: false,
    };
    assert!(input.validate().is_ok());
    for slug in ["", "../private", "Admin", "admin", "bad--slug"] {
        input.slug = slug.into();
        assert!(input.validate().is_err());
    }
}

#[test]
#[ignore = "Requires an explicitly configured disposable blog_test PostgreSQL database"]
fn live_publications_are_isolated_and_subjects_do_not_publish_private_items() {
    let url = std::env::var("BLOG_TEST_DATABASE_URL")
        .expect("Set BLOG_TEST_DATABASE_URL to a disposable migrated blog_test database");
    assert_eq!(url::Url::parse(&url).unwrap().path(), "/blog_test");
    let pool = DbPool::builder()
        .max_size(4)
        .build(ConnectionManager::new(url))
        .unwrap();
    let owner = Uuid::new_v4();
    let journal = Uuid::new_v4();
    let trace = Uuid::new_v4();
    let private = Uuid::new_v4();
    let album = Uuid::new_v4();
    pool.get().unwrap().batch_execute(&format!("INSERT INTO users(id,email,first_name,last_name,handle,password,updated_at)
        VALUES('{owner}','{owner}@test.invalid','Blog','Test','@{owner}','unused',NOW());
        INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status) VALUES('{journal}','{owner}','Test','','','USER_JOURNAL','ACTIVE');
        INSERT INTO traces(id,user_id,journal_id,title,subtitle,content,interaction_date,trace_type,status,start_writing_at,finalized_at)
        VALUES('{trace}','{owner}','{journal}','Public','','First',NOW(),'USER_TRACE','FINALIZED',NOW(),NOW()),
              ('{private}','{owner}','{journal}','Private','','Secret',NOW(),'USER_TRACE','FINALIZED',NOW(),NOW());
        INSERT INTO albums(id,owner_user_id,title,subtitle,content,ordering_mode,completion_status,visibility)
        VALUES('{album}','{owner}','Subject','','','MANUAL','COMPLETE','PRIVATE');
        INSERT INTO album_items(album_id,trace_id,ordering_index) VALUES('{album}','{trace}',0),('{album}','{private}',1);"
    )).unwrap();
    let input = |slug: String| BlogInput {
        slug,
        name: "Notes".into(),
        username: "Pseudonym".into(),
        description: "Public introduction".into(),
        image_asset_id: None,
        enabled: true,
    };
    let blog = save(None, owner, input(format!("notes-{owner}")), &pool).unwrap();
    let other = save(None, owner, input(format!("other-{owner}")), &pool).unwrap();
    let published = publish(&blog, trace, false, &pool).unwrap();
    assert_eq!(
        publish(&blog, trace, false, &pool).unwrap().id,
        published.id
    );
    let elsewhere = publish(&other, trace, false, &pool).unwrap();
    assert_ne!(published.id, elsewhere.id);
    assert!(Post::find_full(published.id, &pool).is_err());
    assert!(Post::find_for_trace_and_publisher(trace, owner, &pool)
        .unwrap()
        .is_none());
    let social = NewPost {
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
    assert_eq!(
        Post::find_for_trace_and_publisher(trace, owner, &pool)
            .unwrap()
            .unwrap()
            .id,
        social.id
    );
    let mut social = social;
    social.status = PostStatus::Published;
    let social = social.update(&pool).unwrap();
    assert_eq!(social.status, PostStatus::Published);
    let subject = publish(&blog, album, true, &pool).unwrap();
    let page = ValidatedPagination {
        offset: 0,
        limit: 20,
    };
    let items = subject_items(&blog, subject.id, page, &pool).unwrap();
    assert_eq!(items.total, 1);
    assert_eq!(items.items[0].source_trace_id, Some(trace));
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET content='Edited' WHERE id='{trace}'"
        ))
        .unwrap();
    assert_eq!(
        post(&blog, published.id, true, &pool).unwrap().content,
        "Edited"
    );
    assert!(pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "INSERT INTO post_grants(post_id,owner_user_id,grantee_scope,access_level,status)
        VALUES('{}','{owner}','ALL_PLATFORM_USERS','READ','ACTIVE')",
            published.id
        ))
        .is_err());
    unpublish(&blog, published.id, &pool).unwrap();
    assert!(post(&blog, published.id, true, &pool).is_err());
    assert_eq!(
        subject_items(&blog, subject.id, page, &pool).unwrap().total,
        0
    );
    assert!(post(&other, elsewhere.id, true, &pool).is_ok());
    let blog = save(
        Some(blog.id),
        owner,
        BlogInput {
            enabled: false,
            ..input(blog.slug.clone())
        },
        &pool,
    )
    .unwrap();
    assert!(public(&blog.slug, &pool).is_err());
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET is_encrypted=true WHERE id='{trace}'"
        ))
        .unwrap();
    assert!(post(&other, elsewhere.id, true, &pool).is_err());
    assert!(publish(&other, trace, false, &pool).is_err());
    pool.get()
        .unwrap()
        .batch_execute(&format!("DELETE FROM users WHERE id='{owner}'"))
        .unwrap();
}
