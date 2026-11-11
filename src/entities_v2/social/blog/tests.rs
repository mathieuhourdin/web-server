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

#[tokio::test]
async fn user_publications_does_not_disclose_other_users_blog_identities() {
    use crate::{entities_v2::session::Session, pagination::PaginationParams};
    use axum::extract::{Extension, Path, Query};
    let pool = DbPool::builder()
        .max_size(1)
        .min_idle(Some(0))
        .build(ConnectionManager::<diesel::PgConnection>::new(
            "postgres://unused/unused",
        ))
        .unwrap();
    let owner = Uuid::new_v4();
    for caller in [None, Some(Uuid::new_v4())] {
        let mut session = Session::anonymous();
        session.user_id = caller;
        let page: PaginationParams = serde_json::from_value(serde_json::json!({})).unwrap();
        let error = super::routes::get_user_publications_route(
            Extension(pool.clone()),
            Extension(session),
            Path(owner),
            Query(page),
        )
        .await
        .err()
        .expect("Only the account owner may list its blog identities");
        assert_eq!(error.status_code, 401);
    }
}

#[test]
fn publication_ordering_patch_distinguishes_missing_null_and_timestamp() {
    use serde_json::json;
    let input: PatchPublicationInput = serde_json::from_value(json!({})).unwrap();
    assert!(input.ordering_date.is_none());
    let input: PatchPublicationInput =
        serde_json::from_value(json!({"ordering_date":null})).unwrap();
    assert_eq!(input.ordering_date, Some(None));
    let input: PatchPublicationInput =
        serde_json::from_value(json!({"ordering_date":"2099-01-01T03:00:00+03:00"})).unwrap();
    assert_eq!(
        input.ordering_date.unwrap().unwrap().to_rfc3339(),
        "2099-01-01T00:00:00+00:00"
    );
    assert!(
        serde_json::from_value::<PatchPublicationInput>(json!({"ordering_date":"not a date"}))
            .is_err()
    );
    assert!(
        serde_json::from_value::<PatchPublicationInput>(json!({"publishing_date":null})).is_err()
    );
}

#[tokio::test]
#[ignore = "Requires an explicitly configured disposable blog_test PostgreSQL database"]
async fn live_publications_are_isolated_and_subjects_do_not_publish_private_items() {
    use diesel::{
        prelude::*,
        sql_types::{Text, Uuid as SqlUuid},
    };
    use diesel_migrations::{embed_migrations, MigrationHarness};
    let url = std::env::var("BLOG_TEST_DATABASE_URL")
        .expect("Set BLOG_TEST_DATABASE_URL to a disposable migrated blog_test database");
    assert_eq!(url::Url::parse(&url).unwrap().path(), "/blog_test");
    let pool = DbPool::builder()
        .max_size(4)
        .build(ConnectionManager::new(url))
        .unwrap();
    const MIGRATIONS: diesel_migrations::EmbeddedMigrations = embed_migrations!();
    let mut conn = pool.get().unwrap();
    let pending = conn.pending_migrations(MIGRATIONS).unwrap();
    let backfill = pending
        .iter()
        .any(|m| m.name().version().to_string().replace('-', "") == "20261111100000");
    for migration in pending
        .iter()
        .filter(|m| m.name().version().to_string().replace('-', "") != "20261111100000")
    {
        conn.run_migration(&**migration).unwrap();
    }
    drop(conn);
    let owner = Uuid::new_v4();
    let journal = Uuid::new_v4();
    let trace = Uuid::new_v4();
    let private = Uuid::new_v4();
    let album = Uuid::new_v4();
    pool.get().unwrap().batch_execute(&format!("INSERT INTO users(id,email,first_name,last_name,handle,password,updated_at)
        VALUES('{owner}','{owner}@test.invalid','Blog','Test','@{owner}','unused',NOW());
        INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status) VALUES('{journal}','{owner}','Test','','','USER_JOURNAL','ACTIVE');
        INSERT INTO traces(id,user_id,journal_id,title,subtitle,content,trace_type,status,start_writing_at,finalized_at)
        VALUES('{trace}','{owner}','{journal}','Public','','First','USER_TRACE','FINALIZED',NOW(),NOW()),
              ('{private}','{owner}','{journal}','Private','','Secret','USER_TRACE','FINALIZED',NOW(),NOW());
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
    let mut blog = save(None, owner, input(format!("notes-{owner}")), &pool).unwrap();
    let other = save(None, owner, input(format!("other-{owner}")), &pool).unwrap();
    let legacy = Uuid::new_v4();
    let archived_legacy = Uuid::new_v4();
    if backfill {
        pool.get().unwrap().batch_execute(&format!("INSERT INTO posts(id,user_id,publication_channel_id,source_trace_id,title,subtitle,content,interaction_type,post_type,status,publishing_date,notification_mode)
            VALUES('{legacy}','{owner}','{}','{trace}','','','','OUTPUT','IDEA','PUBLISHED','2026-01-02','NONE'),
                  ('{archived_legacy}','{owner}','{}','{private}','','','','OUTPUT','IDEA','ARCHIVED','2026-01-03','NONE');",blog.id,blog.id)).unwrap();
    }
    pool.get()
        .unwrap()
        .run_pending_migrations(MIGRATIONS)
        .unwrap();
    let published = publish(&blog, trace, false, &pool).unwrap();
    if backfill {
        assert_eq!(published.id, legacy);
        assert_eq!(
            published.publishing_date.unwrap().date().to_string(),
            "2026-01-02"
        );
        #[derive(QueryableByName)]
        struct Code {
            #[diesel(sql_type=Text)]
            code: String,
        }
        let archived_code =
            diesel::sql_query("SELECT code FROM blog_publication_links WHERE publication_id=$1")
                .bind::<SqlUuid, _>(archived_legacy)
                .get_result::<Code>(&mut *pool.get().unwrap())
                .unwrap()
                .code;
        assert!(super::model::short_target(&archived_code, &pool).is_err());
    }
    assert!(super::urls::valid_short_code(&published.short_code));
    assert_eq!(
        super::model::short_target(&published.short_code, &pool)
            .unwrap()
            .id,
        published.id
    );
    assert_eq!(
        publish(&blog, trace, false, &pool).unwrap().id,
        published.id
    );
    let elsewhere = publish(&other, trace, false, &pool).unwrap();
    assert_ne!(published.id, elsewhere.id);
    assert_ne!(published.short_code, elsewhere.short_code);
    assert_eq!(
        publish(&blog, trace, false, &pool).unwrap().short_code,
        published.short_code
    );
    let response = super::routes::short_link(
        axum::Extension(pool.clone()),
        axum::extract::Path(published.short_code.clone()),
    )
    .await;
    assert_eq!(
        response.status(),
        axum::http::StatusCode::TEMPORARY_REDIRECT
    );
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(
        response.headers()["location"],
        super::urls::publication(&blog.slug, published.id)
    );
    let api = serde_json::to_value(
        post(&blog, published.id, false, &pool)
            .unwrap()
            .with_urls(&blog.slug),
    )
    .unwrap();
    assert_eq!(api["short_url"], super::urls::short(&published.short_code));
    assert_eq!(
        api["public_url"],
        super::urls::publication(&blog.slug, published.id)
    );
    blog = save(
        Some(blog.id),
        owner,
        input(format!("renamed-{owner}")),
        &pool,
    )
    .unwrap();
    assert_eq!(
        super::model::short_target(&published.short_code, &pool)
            .unwrap()
            .slug,
        blog.slug
    );
    assert!(pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE blog_publication_links SET code='AAAAAAAA' WHERE publication_id='{}'",
            published.id
        ))
        .is_err());
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
    let all = owner_publications(owner, page, &pool).unwrap();
    assert_eq!(
        all.total, 3,
        "Two trace posts and one album post, never the social post"
    );
    let mut ids = all
        .items
        .iter()
        .map(|item| item.publication.id)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    let mut expected = vec![published.id, elsewhere.id, subject.id];
    expected.sort_unstable();
    assert_eq!(ids, expected);
    for item in &all.items {
        assert_eq!(item.blog.id, item.publication.publication_channel_id);
        assert_eq!(item.blog.username, "Pseudonym");
        assert!(item.blog.enabled);
    }
    let second = owner_publications(
        owner,
        ValidatedPagination {
            offset: 1,
            limit: 1,
        },
        &pool,
    )
    .unwrap();
    assert_eq!(second.total, 3);
    assert_eq!(second.items[0].publication.id, all.items[1].publication.id);
    let beyond = owner_publications(
        owner,
        ValidatedPagination {
            offset: 99,
            limit: 1,
        },
        &pool,
    )
    .unwrap();
    assert_eq!(beyond.total, 3);
    assert!(beyond.items.is_empty());
    assert_eq!(
        owner_publications(Uuid::new_v4(), page, &pool)
            .unwrap()
            .total,
        0
    );
    let json = serde_json::to_value(&all).unwrap();
    assert!(json["items"][0]["id"].is_string());
    assert!(json["items"][0]["blog"]["slug"].is_string());
    let items = subject_items(&blog, subject.id, page, &pool).unwrap();
    assert_eq!(items.total, 1);
    assert_eq!(items.items[0].source_trace_id, Some(trace));
    // Editorial placement is per blog, not a new publication/distribution.
    let before = posts(&blog, true, page, &pool)
        .unwrap()
        .items
        .into_iter()
        .map(|p| p.id)
        .collect::<Vec<_>>();
    let all_before = owner_publications(owner, page, &pool)
        .unwrap()
        .items
        .into_iter()
        .map(|p| p.publication.id)
        .collect::<Vec<_>>();
    assert_eq!(before[0], subject.id);
    #[derive(QueryableByName)]
    struct Events {
        #[diesel(sql_type=diesel::sql_types::BigInt)]
        count: i64,
    }
    let events = || {
        diesel::sql_query(
            "SELECT count(*)::bigint AS count FROM post_distribution_events WHERE post_id=$1",
        )
        .bind::<SqlUuid, _>(published.id)
        .get_result::<Events>(&mut pool.get().unwrap())
        .unwrap()
        .count
    };
    let event_count = events();
    let ordering_input = || {
        serde_json::from_value::<PatchPublicationInput>(
            serde_json::json!({"ordering_date":"2099-01-01T03:00:00+03:00"}),
        )
        .unwrap()
    };
    assert_eq!(
        patch_publication(&other, published.id, ordering_input(), &pool)
            .err()
            .unwrap()
            .status_code,
        404
    );
    let mut outsider = crate::entities_v2::session::Session::anonymous();
    outsider.user_id = Some(Uuid::new_v4());
    assert!(super::routes::patch_publication(
        axum::Extension(pool.clone()),
        axum::Extension(outsider),
        axum::extract::Path((blog.id, published.id)),
        axum::Json(ordering_input())
    )
    .await
    .is_err());
    let reordered = patch_publication(&blog, published.id, ordering_input(), &pool).unwrap();
    assert!(reordered.ordering_date.is_some());
    assert_eq!(reordered.publishing_date, published.publishing_date);
    assert_eq!(
        reordered.last_distribution_at,
        published.last_distribution_at
    );
    assert_eq!(
        reordered.last_distribution_event_id,
        published.last_distribution_event_id
    );
    assert_eq!(reordered.short_code, published.short_code);
    assert_eq!(events(), event_count);
    for anonymous in [false, true] {
        assert_eq!(
            posts(&blog, anonymous, page, &pool).unwrap().items[0].id,
            published.id
        );
    }
    assert_eq!(
        posts(
            &blog,
            true,
            ValidatedPagination {
                offset: 1,
                limit: 1
            },
            &pool
        )
        .unwrap()
        .items[0]
            .id,
        subject.id
    );
    assert!(post(&other, elsewhere.id, false, &pool)
        .unwrap()
        .ordering_date
        .is_none());
    assert_eq!(
        owner_publications(owner, page, &pool)
            .unwrap()
            .items
            .into_iter()
            .map(|p| p.publication.id)
            .collect::<Vec<_>>(),
        all_before
    );
    assert_eq!(
        patch_publication(
            &blog,
            published.id,
            serde_json::from_value(serde_json::json!({})).unwrap(),
            &pool
        )
        .unwrap()
        .ordering_date,
        reordered.ordering_date
    );
    assert_eq!(
        publish(&blog, trace, false, &pool).unwrap().ordering_date,
        reordered.ordering_date
    );
    let cleared = patch_publication(
        &blog,
        published.id,
        serde_json::from_value(serde_json::json!({"ordering_date":null})).unwrap(),
        &pool,
    )
    .unwrap();
    assert!(cleared.ordering_date.is_none());
    assert_eq!(
        posts(&blog, true, page, &pool)
            .unwrap()
            .items
            .into_iter()
            .map(|p| p.id)
            .collect::<Vec<_>>(),
        before
    );
    assert_eq!(events(), event_count);
    assert!(pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE posts SET ordering_date=NOW() WHERE id='{}'",
            social.id
        ))
        .is_err());
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
    assert!(super::model::short_target(&published.short_code, &pool).is_err());
    let missing = super::routes::short_link(
        axum::Extension(pool.clone()),
        axum::extract::Path(published.short_code.clone()),
    )
    .await;
    assert_eq!(missing.status(), axum::http::StatusCode::NOT_FOUND);
    assert_eq!(missing.headers()["cache-control"], "no-store");
    assert_eq!(
        publish(&blog, trace, false, &pool).unwrap().short_code,
        published.short_code
    );
    unpublish(&blog, published.id, &pool).unwrap();
    assert_eq!(owner_publications(owner, page, &pool).unwrap().total, 2);
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
    assert!(super::model::short_target(&subject.short_code, &pool).is_err());
    let disabled = owner_publications(owner, page, &pool).unwrap();
    assert!(disabled
        .items
        .iter()
        .any(|item| item.blog.id == blog.id && !item.blog.enabled));
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET is_encrypted=true WHERE id='{trace}'"
        ))
        .unwrap();
    assert!(post(&other, elsewhere.id, true, &pool).is_err());
    assert!(super::model::short_target(&elsewhere.short_code, &pool).is_err());
    assert!(publish(&other, trace, false, &pool).is_err());
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE albums SET completion_status='ARCHIVED' WHERE id='{album}'"
        ))
        .unwrap();
    let status = post(&blog, subject.id, false, &pool).unwrap().status;
    assert_eq!(
        patch_publication(&blog, subject.id, ordering_input(), &pool)
            .unwrap()
            .status,
        status
    );
    pool.get()
        .unwrap()
        .batch_execute(&format!("DELETE FROM users WHERE id='{owner}'"))
        .unwrap();
}
