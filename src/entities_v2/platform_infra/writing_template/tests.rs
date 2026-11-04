use super::model::*;
use crate::{
    db::DbPool,
    entities_v2::{
        session::Session,
        trace::{NewTrace, PatchTraceDto, Trace, TraceStatus},
    },
};
use axum::{
    extract::{Extension, Path, Query},
    Json,
};
use diesel::{connection::SimpleConnection, prelude::*, r2d2::ConnectionManager};
use diesel_migrations::{embed_migrations, MigrationHarness};
use uuid::Uuid;

fn input(slug: &str) -> NewTemplate {
    serde_json::from_value(serde_json::json!({
        "slug":slug,"title":"A starting point","content":"First question:\n\nAnother question:",
    }))
    .unwrap()
}

#[test]
fn writing_template_validation_and_trace_patch_contract() {
    let mut template = input("daily_scene");
    assert!(template.validate().is_ok());
    assert_eq!(template.locale, "fr");
    assert_eq!(template.content, "First question:\n\nAnother question:");
    for slug in ["", "Uppercase", "../invalid", "two words"] {
        assert!(input(slug).validate().is_err());
    }
    for locale in ["fr", "en", "fr-CA", "zh-Hant-TW"] {
        assert!(normalize_locale(locale).is_ok());
    }
    for locale in ["", "f", "../../", "fr--CA"] {
        assert!(normalize_locale(locale).is_err());
    }
    assert!(
        serde_json::from_value::<PatchTemplate>(serde_json::json!({"slug":"new-key"})).is_err()
    );
    let id = Uuid::new_v4();
    for (payload, expected) in [
        (serde_json::json!({}), None),
        (serde_json::json!({"writing_template_id":null}), Some(None)),
        (
            serde_json::json!({"writing_template_id":id}),
            Some(Some(id)),
        ),
    ] {
        let patch: PatchTraceDto = serde_json::from_value(payload.clone()).unwrap();
        let put: crate::entities_v2::trace::UpdateTraceDto =
            serde_json::from_value(payload).unwrap();
        assert_eq!(patch.writing_template_id, expected);
        assert_eq!(put.writing_template_id, expected);
    }
}

#[tokio::test]
#[ignore = "Requires explicitly configured disposable blog_test; synthetic users only"]
async fn writing_templates_catalogue_admin_and_trace_provenance() {
    let url = std::env::var("BLOG_TEST_DATABASE_URL").unwrap();
    assert_eq!(url::Url::parse(&url).unwrap().path(), "/blog_test");
    let pool = DbPool::builder()
        .max_size(5)
        .build(ConnectionManager::<PgConnection>::new(url))
        .unwrap();
    const MIGRATIONS: diesel_migrations::EmbeddedMigrations = embed_migrations!();
    pool.get()
        .unwrap()
        .run_pending_migrations(MIGRATIONS)
        .unwrap();
    let owner = Uuid::new_v4();
    let stranger = Uuid::new_v4();
    let journal = Uuid::new_v4();
    pool.get().unwrap().batch_execute(&format!("INSERT INTO users(id,email,first_name,last_name,handle,password,updated_at)
        VALUES('{owner}','{owner}@test.invalid','Synthetic','Writer','{owner}','unused',NOW()),
              ('{stranger}','{stranger}@test.invalid','Synthetic','Other','{stranger}','unused',NOW());
        INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status)
        VALUES('{journal}','{owner}','Synthetic','','','USER_JOURNAL','ACTIVE');")).unwrap();
    let query = || {
        serde_json::from_value::<ListQuery>(serde_json::json!({"limit":100,"locale":"fr"})).unwrap()
    };
    assert_eq!(
        super::routes::list(
            Extension(pool.clone()),
            Extension(Session::anonymous()),
            Query(query())
        )
        .await
        .unwrap_err()
        .status_code,
        401
    );
    let mut session = Session::anonymous();
    session.user_id = Some(owner);
    assert_eq!(
        super::routes::create(
            Extension(pool.clone()),
            Extension(session.clone()),
            Json(input("not_admin"))
        )
        .await
        .unwrap_err()
        .status_code,
        403
    );
    let catalogue = super::routes::list(
        Extension(pool.clone()),
        Extension(session.clone()),
        Query(query()),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(catalogue.total, 21);
    assert_eq!(catalogue.items[0].slug, "last_night_dream");
    pool.get()
        .unwrap()
        .batch_execute(&format!(
            "INSERT INTO user_roles(user_id,role) VALUES('{owner}','ADMIN')"
        ))
        .unwrap();
    let slug = format!("synthetic_{}", Uuid::new_v4().simple());
    let (status, Json(template)) = super::routes::create(
        Extension(pool.clone()),
        Extension(session.clone()),
        Json(input(&slug)),
    )
    .await
    .unwrap();
    assert_eq!(status, axum::http::StatusCode::CREATED);

    let draft = NewTrace::new(
        "Synthetic".into(),
        "".into(),
        "Original content".into(),
        chrono::Utc::now().naive_utc(),
        owner,
        journal,
    )
    .create(&pool)
    .unwrap();
    let patch = |version, id, content| {
        serde_json::from_value::<PatchTraceDto>(serde_json::json!({
            "expected_version_integer":version,"writing_template_id":id,"content":content,
        }))
        .unwrap()
    };
    let Json(linked) = crate::entities_v2::trace::patch_trace_route(
        Extension(pool.clone()),
        Extension(session.clone()),
        Path(draft.id),
        Json(patch(
            draft.version_integer,
            Some(template.id),
            "Customised text",
        )),
    )
    .await
    .unwrap();
    assert_eq!(linked.writing_template_id, Some(template.id));
    assert_eq!(linked.content, "Customised text");
    assert_eq!(linked.version_integer, draft.version_integer + 1);
    // A stale save never clears the newer association or overwrites content.
    let stale = crate::entities_v2::trace::patch_trace_route(
        Extension(pool.clone()),
        Extension(session.clone()),
        Path(draft.id),
        Json(patch(draft.version_integer, None, "Stale text")),
    )
    .await
    .unwrap_err();
    assert_eq!(stale.status_code, 409);
    assert_eq!(
        Trace::find_full_trace(draft.id, &pool)
            .unwrap()
            .writing_template_id,
        Some(template.id)
    );
    let mut foreign = Session::anonymous();
    foreign.user_id = Some(stranger);
    assert_eq!(
        crate::entities_v2::trace::patch_trace_route(
            Extension(pool.clone()),
            Extension(foreign),
            Path(draft.id),
            Json(patch(linked.version_integer, None, "Bad"))
        )
        .await
        .unwrap_err()
        .status_code,
        401
    );

    let Json(inactive) = super::routes::patch(
        Extension(pool.clone()),
        Extension(session.clone()),
        Path(template.id),
        Json(PatchTemplate {
            content: Some("Changed catalogue text".into()),
            is_active: Some(false),
            ..Default::default()
        }),
    )
    .await
    .unwrap();
    assert!(!inactive.is_active);
    let catalogue = super::routes::list(
        Extension(pool.clone()),
        Extension(session.clone()),
        Query(query()),
    )
    .await
    .unwrap()
    .0;
    assert!(!catalogue.items.iter().any(|t| t.id == template.id));
    assert!(super::routes::get(
        Extension(pool.clone()),
        Extension(session.clone()),
        Path(template.id)
    )
    .await
    .is_ok());
    let current = Trace::find_full_trace(draft.id, &pool).unwrap();
    assert_eq!(current.content, "Customised text"); // no live template content.
    let Json(edited) = crate::entities_v2::trace::patch_trace_route(
        Extension(pool.clone()),
        Extension(session.clone()),
        Path(draft.id),
        Json(
            serde_json::from_value(serde_json::json!({
                "expected_version_integer":current.version_integer,"content":"Still my words"
            }))
            .unwrap(),
        ),
    )
    .await
    .unwrap();
    assert_eq!(edited.writing_template_id, Some(template.id));
    let Json(cleared) = crate::entities_v2::trace::patch_trace_route(
        Extension(pool.clone()),
        Extension(session.clone()),
        Path(draft.id),
        Json(patch(edited.version_integer, None, "Still my words")),
    )
    .await
    .unwrap();
    assert_eq!(cleared.writing_template_id, None);
    assert!(crate::entities_v2::trace::patch_trace_route(
        Extension(pool.clone()),
        Extension(session.clone()),
        Path(draft.id),
        Json(patch(
            cleared.version_integer,
            Some(template.id),
            "Must not be saved"
        ))
    )
    .await
    .is_err());
    assert_eq!(
        Trace::find_full_trace(draft.id, &pool).unwrap().content,
        "Still my words"
    );
    assert!(crate::entities_v2::trace::patch_trace_route(
        Extension(pool.clone()),
        Extension(session.clone()),
        Path(draft.id),
        Json(patch(
            cleared.version_integer,
            Some(Uuid::new_v4()),
            "Unknown"
        ))
    )
    .await
    .is_err());
    let _ = super::routes::patch(
        Extension(pool.clone()),
        Extension(session.clone()),
        Path(template.id),
        Json(PatchTemplate {
            is_active: Some(true),
            ..Default::default()
        }),
    )
    .await
    .unwrap();
    let mut final_trace = Trace::find_full_trace(draft.id, &pool).unwrap();
    final_trace.writing_template_id = Some(template.id);
    let final_trace = final_trace
        .set_status(TraceStatus::Finalized, &pool)
        .unwrap();
    assert_eq!(final_trace.writing_template_id, Some(template.id));
    let mut changed = final_trace.clone();
    changed.writing_template_id = None;
    assert!(changed
        .update_with_expected_version(final_trace.version_integer, &pool)
        .is_err());
    let batch = Trace::find_full_traces_by_ids(&[final_trace.id], &pool).unwrap();
    assert_eq!(batch[0].writing_template_id, Some(template.id));
    let collection = Trace::get_all_for_user(owner, &pool).unwrap();
    assert_eq!(collection[0].writing_template_id, Some(template.id));
    let (collection, _) = Trace::get_for_journal_paginated(
        journal,
        owner,
        0,
        20,
        None,
        TraceStatus::Finalized,
        None,
        &pool,
    )
    .unwrap();
    assert_eq!(collection[0].writing_template_id, Some(template.id));
    let (feed, _) =
        crate::entities_v2::feed::hydrate::find_my_feed_items_paginated(owner, None, 0, 20, &pool)
            .unwrap();
    assert_eq!(feed[0].writing_template_id, Some(template.id));
    let mut created = NewTrace::new(
        "Synthetic".into(),
        "".into(),
        "From creation".into(),
        chrono::Utc::now().naive_utc(),
        owner,
        journal,
    );
    created.writing_template_id = Some(template.id);
    assert_eq!(
        created.create_finalized(&pool).unwrap().writing_template_id,
        Some(template.id)
    );
    pool.get().unwrap().batch_execute(&format!("DELETE FROM users WHERE id IN ('{owner}','{stranger}'); DELETE FROM writing_templates WHERE id='{}'",template.id)).unwrap();
}
