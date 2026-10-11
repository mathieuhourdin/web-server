use super::{model::*, persist};
use crate::db::DbPool;
use crate::entities_v2::{
    journal::Journal,
    post::Post,
    post_grant::{NewPostGrantDto, PostGrant},
    trace::{Trace, TraceComplementAudienceMode},
};
use diesel::{
    connection::SimpleConnection,
    prelude::*,
    r2d2::ConnectionManager,
    sql_types::{Array, Uuid as SqlUuid},
};
use std::sync::{Arc, Barrier};
use uuid::Uuid;

#[test]
fn validates_audience_names() {
    assert_eq!(persist::validate_name(" Friends ").unwrap(), "Friends");
    assert!(persist::validate_name(" ").is_err());
    assert!(persist::validate_name(&"a".repeat(101)).is_err());
}

#[test]
fn removed_audience_role_is_rejected_instead_of_silently_broadening_public_links() {
    use crate::entities_v2::post::{routes::PutTracePostDto, NewPostDto};
    assert!(serde_json::from_str::<PutTracePostDto>(r#"{"status":"published"}"#).is_ok());
    for role in [r#""restricted""#, r#""default""#, "null"] {
        let payload = format!(r#"{{"status":"published","audience_role":{role}}}"#);
        assert!(serde_json::from_str::<PutTracePostDto>(&payload).is_err());
        assert!(serde_json::from_str::<NewPostDto>(&payload).is_err());
    }
}

#[test]
fn member_removal_defaults_are_future_only() {
    let query: RemoveMemberQuery = serde_json::from_str("{}").unwrap();
    assert!(query.stop_future);
    assert!(!query.revoke_history);
    let query: RemoveMemberQuery =
        serde_json::from_str(r#"{"stop_future":false,"revoke_history":true}"#).unwrap();
    assert!(!query.stop_future);
    assert!(query.revoke_history);
}

struct Fixture {
    pool: DbPool,
    owner: Uuid,
    reader: Uuid,
    stranger: Uuid,
    journal: Uuid,
}

impl Fixture {
    fn new() -> Self {
        let url = std::env::var("AUDIENCE_TEST_DATABASE_URL").expect(
            "Set AUDIENCE_TEST_DATABASE_URL to a disposable migrated audience_test database",
        );
        let parsed = url::Url::parse(&url).unwrap();
        assert_eq!(
            parsed.path(),
            "/audience_test",
            "Integration fixtures must only use the disposable audience_test database"
        );
        let pool = DbPool::builder()
            .max_size(8)
            .build(ConnectionManager::new(url))
            .unwrap();
        let owner = Uuid::new_v4();
        let reader = Uuid::new_v4();
        let stranger = Uuid::new_v4();
        let journal = Uuid::new_v4();
        pool.get().unwrap().batch_execute(&format!("INSERT INTO users(id,email,first_name,last_name,handle,password,updated_at)
            VALUES('{owner}','{owner}@test.invalid','Owner','Test','@{owner}','unused',NOW()),
                ('{reader}','{reader}@test.invalid','Reader','Test','@{reader}','unused',NOW()),
                ('{stranger}','{stranger}@test.invalid','Stranger','Test','@{stranger}','unused',NOW());
            INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status) VALUES('{journal}','{owner}','Test','','','USER_JOURNAL','ACTIVE');
            INSERT INTO relationships(requester_user_id,target_user_id,relationship_type,status,accepted_at)
            VALUES('{reader}','{owner}','FOLLOW','ACCEPTED',NOW());")).unwrap();
        Self {
            pool,
            owner,
            reader,
            stranger,
            journal,
        }
    }

    fn publication(&self) -> (Uuid, Uuid) {
        let trace = Uuid::new_v4();
        let post = Uuid::new_v4();
        self.pool.get().unwrap().batch_execute(&format!("INSERT INTO traces(id,user_id,journal_id,title,subtitle,content,interaction_date,trace_type,status,start_writing_at,finalized_at)
            VALUES('{trace}','{}','{}','Test','','Content',NOW(),'USER_TRACE','FINALIZED',NOW(),NOW());
            INSERT INTO posts(id,user_id,source_trace_id,title,subtitle,content,interaction_type,post_type,status,publishing_date)
            VALUES('{post}','{}','{trace}','','','','OUTPUT','IDEA','PUBLISHED',NOW());",self.owner,self.journal,self.owner)).unwrap();
        (trace, post)
    }

    fn grant(&self, post: Uuid) -> PostGrant {
        PostGrant::find_for_post_paginated(post, 0, 100, &self.pool)
            .unwrap()
            .0
            .into_iter()
            .find(|g| g.grantee_user_id == Some(self.reader))
            .unwrap()
    }

    fn can_read(&self, post: Uuid) -> bool {
        PostGrant::user_can_read_post(
            &Post::find_full(post, &self.pool).unwrap(),
            self.reader,
            &self.pool,
        )
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Ok(mut conn) = self.pool.get() {
            let _ = diesel::sql_query("DELETE FROM users WHERE id=ANY($1)")
                .bind::<Array<SqlUuid>, _>(vec![self.owner, self.reader, self.stranger])
                .execute(&mut conn);
        }
    }
}

#[tokio::test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
async fn trace_albums_route_respects_trace_and_album_access_and_pagination() {
    use crate::{
        entities_v2::{
            album::{routes::get_trace_albums_route, Album},
            session::Session,
        },
        pagination::{PaginatedResponse, PaginationParams},
    };
    use axum::{
        extract::{Extension, Path, Query},
        Json,
    };
    let f = Fixture::new();
    let (tid, pid) = f.publication();
    let private = Uuid::new_v4();
    let published = Uuid::new_v4();
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "INSERT INTO albums(id,owner_user_id,title,visibility) VALUES
            ('{private}','{}','Private album','PRIVATE'),
            ('{published}','{}','Published album','PUBLISHED');
         INSERT INTO album_items(album_id,trace_id,ordering_index) VALUES
            ('{private}','{tid}',0),('{published}','{tid}',0);",
            f.owner, f.owner
        ))
        .unwrap();
    async fn page(
        f: &Fixture,
        user: Uuid,
        trace: Uuid,
        offset: i64,
        limit: i64,
    ) -> Result<Json<PaginatedResponse<Album>>, crate::entities_v2::error::PpdcError> {
        let mut session = Session::anonymous();
        session.user_id = Some(user);
        get_trace_albums_route(
            Extension(f.pool.clone()),
            Extension(session),
            Path(trace),
            Query(PaginationParams::from_parts(Some(offset), Some(limit))),
        )
        .await
    }
    let first = page(&f, f.owner, tid, 0, 1).await.unwrap().0;
    let second = page(&f, f.owner, tid, 1, 1).await.unwrap().0;
    assert_eq!(first.total, 2);
    assert_eq!(first.items.len(), 1);
    assert_eq!(second.items.len(), 1);
    assert_ne!(first.items[0].id, second.items[0].id);
    assert_eq!(
        page(&f, f.reader, tid, 0, 20)
            .await
            .err()
            .unwrap()
            .status_code,
        401
    );
    PostGrant::create_or_update(
        &Post::find_full(pid, &f.pool).unwrap(),
        f.owner,
        NewPostGrantDto {
            grantee_user_id: Some(f.reader),
            grantee_scope: None,
            access_level: None,
        },
        &f.pool,
    )
    .unwrap();
    let reader = page(&f, f.reader, tid, 0, 20).await.unwrap().0;
    assert_eq!(reader.total, 1);
    assert_eq!(reader.items[0].id, published);
    assert_eq!(
        page(&f, f.stranger, tid, 0, 20)
            .await
            .err()
            .unwrap()
            .status_code,
        401
    );
    let (empty, _) = f.publication();
    assert_eq!(page(&f, f.owner, empty, 0, 20).await.unwrap().0.total, 0);
}

#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn shared_journal_follower_automation_uses_audiences_only() {
    let f = Fixture::new();
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE journals SET sharing_mode='SHARED' WHERE id='{}';",
            f.journal
        ))
        .unwrap();
    let (_, historical) = f.publication();
    let journal = Journal::find_full(f.journal, &f.pool).unwrap();
    let pending =
        persist::create_missing_members_for_existing_followers(&journal, &f.pool).unwrap();
    assert_eq!(pending.len(), 1);
    let (_, aid, reader) = pending[0];
    assert_eq!(reader, f.reader);
    assert!(
        !f.can_read(historical),
        "Auto membership must not share history"
    );
    assert!(
        persist::create_missing_members_for_existing_followers(&journal, &f.pool)
            .unwrap()
            .is_empty()
    );
    let (_, future) = f.publication();
    assert!(!f.can_read(future));
    persist::attach(f.owner, aid, future, &f.pool).unwrap();
    assert!(f.can_read(future));
    persist::remove_member(f.owner, aid, reader, true, false, &f.pool).unwrap();
    let mut conn = f.pool.get().unwrap();
    assert!(
        persist::create_members_for_new_follower_pair_with_conn(f.owner, reader, &mut conn)
            .unwrap()
            .is_empty()
    );
    drop(conn);
    assert!(
        !persist::member(&mut f.pool.get().unwrap(), aid, reader)
            .unwrap()
            .future_access_enabled
    );
    persist::replace_defaults(f.owner, f.journal, &[], &f.pool).unwrap();
    assert!(
        persist::create_missing_members_for_existing_followers(&journal, &f.pool)
            .unwrap()
            .is_empty()
    );
    use crate::schema::journal_sharing_policies;
    let policies = journal_sharing_policies::table
        .filter(journal_sharing_policies::journal_id.eq(f.journal))
        .count()
        .get_result::<i64>(&mut f.pool.get().unwrap())
        .unwrap();
    assert_eq!(
        policies, 0,
        "No retired policies may be created by automation"
    );
}

#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn public_links_use_sensitivity_independently_of_grants() {
    let f = Fixture::new();
    let (tid, normal) = f.publication();
    assert!(
        !f.can_read(normal),
        "Public links are independent of individual grants"
    );
    assert_eq!(
        Post::find_public_default_for_journal_paginated(f.journal, 0, 20, &f.pool)
            .unwrap()
            .1,
        1
    );
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET sharing_sensitivity='SENSITIVE' WHERE id='{tid}';"
        ))
        .unwrap();
    assert_eq!(
        Post::find_public_default_for_journal_paginated(f.journal, 0, 20, &f.pool)
            .unwrap()
            .1,
        0
    );
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET sharing_sensitivity='NORMAL',is_encrypted=TRUE WHERE id='{tid}';"
        ))
        .unwrap();
    let error = f
        .pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE posts SET status='PUBLISHED' WHERE id='{normal}';"
        ))
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("Source does not permit publication"));
    assert_eq!(
        Post::find_public_default_for_journal_paginated(f.journal, 0, 20, &f.pool)
            .unwrap()
            .1,
        0
    );
}

#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn only_me_deletes_preparation_but_preserves_publication_history() {
    use crate::entities_v2::post::PostStatus;
    let f = Fixture::new();
    let (_, published) = f.publication();
    assert_eq!(
        Post::find_full(published, &f.pool)
            .unwrap()
            .delete_draft_trace_post(&f.pool)
            .unwrap_err()
            .status_code,
        400
    );
    let mut history = Post::find_full(published, &f.pool).unwrap();
    history.status = PostStatus::Draft;
    history = history.update(&f.pool).unwrap();
    assert_eq!(
        history
            .delete_draft_trace_post(&f.pool)
            .unwrap_err()
            .status_code,
        400
    );
    let (tid, prepared) = f.publication();
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE posts SET status='DRAFT',publishing_date=NULL WHERE id='{prepared}';"
        ))
        .unwrap();
    let a = persist::create(f.owner, "Delete preparation", &f.pool).unwrap();
    persist::add_member(f.owner, a.id, f.reader, true, &f.pool).unwrap();
    persist::attach(f.owner, a.id, prepared, &f.pool).unwrap();
    assert_eq!(
        PostGrant::find_for_post_paginated(prepared, 0, 100, &f.pool)
            .unwrap()
            .1,
        1
    );
    Post::find_full(prepared, &f.pool)
        .unwrap()
        .delete_draft_trace_post(&f.pool)
        .unwrap();
    assert!(Post::find_for_trace_and_publisher(tid, f.owner, &f.pool)
        .unwrap()
        .is_none());
    assert_eq!(
        PostGrant::find_for_post_paginated(prepared, 0, 100, &f.pool)
            .unwrap()
            .1,
        0
    );
    assert!(
        Trace::find_full_trace(tid, &f.pool).is_ok(),
        "Deleting preparation must preserve the trace"
    );
}

#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn publication_uses_only_prepared_grants() {
    use crate::entities_v2::post::PostStatus;
    let f = Fixture::new();
    let a = persist::create(f.owner, "Prepared", &f.pool).unwrap();
    persist::add_member(f.owner, a.id, f.reader, true, &f.pool).unwrap();
    persist::replace_defaults(f.owner, f.journal, &[a.id], &f.pool).unwrap();
    let (tid, pid) = f.publication();
    assert!(
        !f.can_read(pid),
        "Publication never applies journal defaults"
    );
    assert!(persist::post_audiences(f.owner, pid, &f.pool)
        .unwrap()
        .is_empty());
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET status='DRAFT',finalized_at=NULL WHERE id='{tid}';
         UPDATE posts SET status='DRAFT',publishing_date=NULL WHERE id='{pid}';"
        ))
        .unwrap();
    let attached = persist::attach(f.owner, a.id, pid, &f.pool).unwrap();
    assert_eq!(attached.newly_shared_grant_count, 1);
    assert_eq!(f.grant(pid).audience_id, Some(a.id));
    assert!(!f.can_read(pid), "Prepared grants never expose drafts");
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET status='DRAFT',content='Autosaved draft' WHERE id='{tid}';"
        ))
        .unwrap();
    assert_eq!(
        Post::find_full(pid, &f.pool).unwrap().status,
        PostStatus::Draft,
        "Autosaving a draft must preserve its staged publication"
    );
    f.pool.get().unwrap().batch_execute(&format!(
        "INSERT INTO relationships(requester_user_id,target_user_id,relationship_type,status,accepted_at)
         VALUES('{}','{}','FOLLOW','ACCEPTED',NOW());", f.stranger,f.owner
    )).unwrap();
    persist::add_member(f.owner, a.id, f.stranger, true, &f.pool).unwrap();
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET status='FINALIZED',finalized_at=NOW() WHERE id='{tid}';"
        ))
        .unwrap();
    assert!(!f.can_read(pid));
    let mut post = Post::find_full(pid, &f.pool).unwrap();
    post.status = PostStatus::Published;
    post = post.update(&f.pool).unwrap();
    assert!(f.can_read(pid));
    assert_eq!(
        PostGrant::find_for_post_paginated(pid, 0, 100, &f.pool)
            .unwrap()
            .1,
        1,
        "Publication must not expand audience membership"
    );
    PostGrant::revoke(pid, f.grant(pid).id, f.owner, &f.pool).unwrap();
    assert!(
        persist::post_audiences(f.owner, pid, &f.pool)
            .unwrap()
            .is_empty(),
        "Removing the last audience-origin reader also removes its history scope"
    );
    assert!(!f.can_read(pid));
    post.status = PostStatus::Archived;
    post = post.update(&f.pool).unwrap();
    post.status = PostStatus::Published;
    post.update(&f.pool).unwrap();
    assert!(
        !f.can_read(pid),
        "Republishing never restores withdrawn grants"
    );
    persist::detach(f.owner, a.id, pid, true, &f.pool).unwrap();
    assert_eq!(
        PostGrant::find_for_post_paginated(pid, 0, 100, &f.pool)
            .unwrap()
            .0
            .iter()
            .filter(|g| g.status == crate::entities_v2::post_grant::PostGrantStatus::Active)
            .count(),
        0
    );
}

#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn audience_trace_counts_match_the_eligible_collection() {
    let f = Fixture::new();
    let a = persist::create(f.owner, "Counted", &f.pool).unwrap();
    let empty = persist::create(f.owner, "Empty", &f.pool).unwrap();
    assert_eq!(
        persist::with_trace_counts(vec![a.clone()], &f.pool).unwrap()[0].trace_count,
        0
    );
    assert!(persist::with_trace_counts(Vec::new(), &f.pool)
        .unwrap()
        .is_empty());

    let (_, published) = f.publication();
    persist::attach(f.owner, a.id, published, &f.pool).unwrap();
    let (_, staged) = f.publication();
    let (_, archived) = f.publication();
    let (encrypted_trace, encrypted_post) = f.publication();
    let (archived_trace, archived_trace_post) = f.publication();
    for pid in [staged, archived, encrypted_post, archived_trace_post] {
        persist::attach(f.owner, a.id, pid, &f.pool).unwrap();
    }
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE posts SET status='DRAFT' WHERE id='{staged}';
         UPDATE posts SET status='ARCHIVED' WHERE id='{archived}';
         UPDATE traces SET is_encrypted=TRUE WHERE id='{encrypted_trace}';
         UPDATE traces SET status='ARCHIVED' WHERE id='{archived_trace}';"
        ))
        .unwrap();

    let (audiences, total) = persist::list(f.owner, false, 0, 20, &f.pool).unwrap();
    assert_eq!(total, 2);
    let views = persist::with_trace_counts(audiences, &f.pool).unwrap();
    assert_eq!(
        views
            .iter()
            .find(|v| v.audience.id == a.id)
            .unwrap()
            .trace_count,
        1
    );
    assert_eq!(
        views
            .iter()
            .find(|v| v.audience.id == empty.id)
            .unwrap()
            .trace_count,
        0
    );
    let listed_total = persist::list_posts(f.owner, a.id, None, 0, 20, &f.pool)
        .unwrap()
        .1;
    assert_eq!(listed_total, 1);
    let owned = persist::owned(&mut *f.pool.get().unwrap(), f.owner, a.id).unwrap();
    let detail = persist::with_trace_counts(vec![owned], &f.pool)
        .unwrap()
        .remove(0);
    assert_eq!(detail.trace_count, listed_total);
    let json = serde_json::to_value(detail).unwrap();
    assert_eq!(json["id"], serde_json::json!(a.id));
    assert_eq!(json["trace_count"], 1);
    assert!(json.get("audience").is_none(), "Read projection stays flat");
    assert_eq!(
        persist::owned(&mut *f.pool.get().unwrap(), f.stranger, a.id)
            .unwrap_err()
            .status_code,
        404
    );

    // Removing a post association updates the query-time count immediately.
    persist::detach(f.owner, a.id, published, false, &f.pool).unwrap();
    assert_eq!(
        persist::with_trace_counts(vec![a], &f.pool).unwrap()[0].trace_count,
        0
    );
}

#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn individual_removal_revokes_all_sources_and_detaches_empty_origins() {
    let f = Fixture::new();
    let a = persist::create(f.owner, "Origin", &f.pool).unwrap();
    let b = persist::create(f.owner, "Overlapping scope", &f.pool).unwrap();
    f.pool.get().unwrap().batch_execute(&format!(
        "INSERT INTO relationships(requester_user_id,target_user_id,relationship_type,status,accepted_at)
         VALUES('{}','{}','FOLLOW','ACCEPTED',NOW())", f.stranger, f.owner)).unwrap();
    for user in [f.reader, f.stranger] {
        persist::add_member(f.owner, a.id, user, true, &f.pool).unwrap();
        persist::add_member(f.owner, b.id, user, true, &f.pool).unwrap();
    }
    let (_, pid) = f.publication();
    persist::attach(f.owner, a.id, pid, &f.pool).unwrap();
    persist::attach(f.owner, b.id, pid, &f.pool).unwrap();
    let reader = f.grant(pid);
    assert!(PostGrant::revoke(pid, reader.id, f.stranger, &f.pool).is_err());
    PostGrant::revoke(pid, reader.id, f.owner, &f.pool).unwrap();
    assert_eq!(
        persist::post_audiences(f.owner, pid, &f.pool)
            .unwrap()
            .len(),
        2
    );
    // Finish removing the two readers concurrently, including an idempotent DELETE.
    let other = PostGrant::find_for_post_paginated(pid, 0, 100, &f.pool)
        .unwrap()
        .0
        .into_iter()
        .find(|g| g.grantee_user_id == Some(f.stranger))
        .unwrap();
    let results = std::thread::scope(|s| {
        let first = s.spawn(|| PostGrant::revoke(pid, reader.id, f.owner, &f.pool));
        let last = s.spawn(|| PostGrant::revoke(pid, other.id, f.owner, &f.pool));
        (first.join().unwrap(), last.join().unwrap())
    });
    assert!(results.0.is_ok() && results.1.is_ok());
    let remaining = persist::post_audiences(f.owner, pid, &f.pool).unwrap();
    assert!(
        remaining.is_empty(),
        "Both active origins lose their last reader"
    );
    assert!(f.grant(pid).sources.iter().all(|s| s.status == "revoked"));
    // Explicit selection again restores grants, rather than publication doing so.
    persist::attach(f.owner, a.id, pid, &f.pool).unwrap();
    assert!(f.can_read(pid));
    let barrier = Arc::new(Barrier::new(2));
    std::thread::scope(|s| {
        for id in [reader.id, other.id] {
            let barrier = barrier.clone();
            let f = &f;
            s.spawn(move || {
                barrier.wait();
                PostGrant::revoke(pid, id, f.owner, &f.pool).unwrap();
            });
        }
    });
    assert_eq!(
        persist::post_audiences(f.owner, pid, &f.pool)
            .unwrap()
            .len(),
        0
    );
}

#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn audience_access_and_history_integration() {
    let f = Fixture::new();
    let a = persist::create(f.owner, "Friends", &f.pool).unwrap();
    let b = persist::create(f.owner, "Family", &f.pool).unwrap();
    let (_, past) = f.publication();
    persist::attach(f.owner, a.id, past, &f.pool).unwrap();
    persist::attach(f.owner, b.id, past, &f.pool).unwrap();
    assert_eq!(
        persist::rename(f.stranger, a.id, "Stolen", &f.pool)
            .unwrap_err()
            .status_code,
        404
    );
    assert_eq!(
        persist::add_member(f.owner, a.id, f.stranger, true, &f.pool)
            .unwrap_err()
            .status_code,
        403
    );
    assert!(
        persist::add_member(f.owner, a.id, f.reader, true, &f.pool)
            .unwrap()
            .1
    );
    persist::add_member(f.owner, b.id, f.reader, true, &f.pool).unwrap();
    assert!(
        !f.can_read(past),
        "Joining must not share history automatically"
    );
    let (_, pending) = persist::pending(f.owner, 0, 100, &f.pool).unwrap();
    assert_eq!(pending, 2);
    let (preview, total) =
        persist::list_posts(f.owner, a.id, Some(f.reader), 0, 1, &f.pool).unwrap();
    assert_eq!(total, 1);
    assert!(!preview[0].has_access);
    let history = || HistoryDecisionDto {
        decision: JournalHistoryDecision::AllNormal,
        post_ids: None,
    };
    assert_eq!(
        persist::apply_history(f.owner, a.id, f.reader, history(), &f.pool)
            .unwrap()
            .newly_shared_post_count,
        1
    );
    assert_eq!(
        persist::apply_history(f.owner, b.id, f.reader, history(), &f.pool)
            .unwrap()
            .newly_shared_post_count,
        0
    );
    assert_eq!(f.grant(past).audience_id, Some(a.id));
    assert!(f.can_read(past));
    let removal = persist::remove_member(f.owner, a.id, f.reader, true, false, &f.pool).unwrap();
    assert_eq!(removal.revoked_post_count, 0);
    assert!(
        f.can_read(past),
        "Future-only removal must preserve existing access"
    );
    persist::change_future(f.owner, a.id, f.reader, true, &f.pool).unwrap();
    assert_eq!(
        persist::remove_member(f.owner, b.id, f.reader, false, true, &f.pool)
            .unwrap()
            .revoked_post_count,
        0
    );
    assert!(
        f.can_read(past),
        "Removing one audience preserves the other active source"
    );
    assert_eq!(
        persist::remove_member(f.owner, a.id, f.reader, false, true, &f.pool)
            .unwrap()
            .revoked_post_count,
        1
    );
    assert!(!f.can_read(past));
    f.pool.get().unwrap().batch_execute(&format!("UPDATE posts SET status='ARCHIVED' WHERE id='{past}'; UPDATE posts SET status='PUBLISHED' WHERE id='{past}';")).unwrap();
    assert!(
        !f.can_read(past),
        "Automatic audience processing must not restore revoked history"
    );
    persist::restart_history_review(f.owner, b.id, f.reader, &f.pool).unwrap();
    persist::apply_history(f.owner, b.id, f.reader, history(), &f.pool).unwrap();
    assert_eq!(
        f.grant(past).audience_id,
        Some(a.id),
        "Legacy origin metadata is not rewritten by new source decisions"
    );
    assert_eq!(
        persist::remove_member(f.owner, b.id, f.reader, false, true, &f.pool)
            .unwrap()
            .revoked_post_count,
        1
    );
    assert!(!f.can_read(past));
    persist::remove_member(f.owner, a.id, f.reader, false, true, &f.pool).unwrap();
    persist::remove_member(f.owner, a.id, f.reader, true, false, &f.pool).unwrap();
    persist::replace_defaults(f.owner, f.journal, &[a.id, b.id], &f.pool).unwrap();
    let (future_trace, future) = f.publication();
    assert!(!f.can_read(future));
    persist::attach(f.owner, b.id, future, &f.pool).unwrap();
    persist::attach(f.owner, a.id, future, &f.pool).unwrap();
    assert!(f.can_read(future));
    assert_eq!(f.grant(future).audience_id, Some(b.id));
    let (readers, total) =
        Journal::find_audience_paginated(f.journal, f.owner, 0, 20, &f.pool).unwrap();
    assert_eq!(total, 1);
    assert!(readers[0].has_future_default_access);
    assert_eq!(readers[0].default_audience_ids, vec![b.id]);
    assert!(!f.can_read(past));
    assert_eq!(
        persist::post_audiences(f.owner, future, &f.pool)
            .unwrap()
            .len(),
        2
    );
    let (_, manual) = f.publication();
    PostGrant::create_or_update(
        &Post::find_full(manual, &f.pool).unwrap(),
        f.owner,
        NewPostGrantDto {
            grantee_user_id: Some(f.reader),
            grantee_scope: None,
            access_level: None,
        },
        &f.pool,
    )
    .unwrap();
    persist::attach(f.owner, b.id, manual, &f.pool).unwrap();
    assert_eq!(
        f.grant(manual).audience_id,
        None,
        "Manual first-source must remain manual"
    );
    let child = Trace::create_complement(
        future_trace,
        f.owner,
        String::new(),
        String::new(),
        "PS".to_string(),
        TraceComplementAudienceMode::Parent,
        &[],
        &f.pool,
    )
    .unwrap();
    assert_eq!(f.grant(child.post.id).audience_id, Some(b.id));
    assert!(f.can_read(child.post.id));
    persist::remove_member(f.owner, b.id, f.reader, false, true, &f.pool).unwrap();
    assert!(!f.can_read(future));
    assert!(!f.can_read(child.post.id));
    assert!(f.can_read(manual));
    persist::restart_history_review(f.owner, a.id, f.reader, &f.pool).unwrap();
    assert_eq!(
        persist::apply_history(f.owner, a.id, f.reader, history(), &f.pool)
            .unwrap()
            .newly_shared_post_count,
        2
    );
    assert_eq!(
        persist::apply_history(f.owner, a.id, f.reader, history(), &f.pool)
            .unwrap()
            .newly_shared_post_count,
        0
    );
    persist::restart_history_review(f.owner, a.id, f.reader, &f.pool).unwrap();
    assert_eq!(
        persist::apply_history(
            f.owner,
            a.id,
            f.reader,
            HistoryDecisionDto {
                decision: JournalHistoryDecision::UserSelected,
                post_ids: Some(vec![Uuid::new_v4()])
            },
            &f.pool
        )
        .unwrap_err()
        .status_code,
        400
    );
    // Detachment future-only leaves its already materialized grants untouched.
    persist::detach(f.owner, a.id, past, false, &f.pool).unwrap();
    assert!(f.can_read(past));
    // Unfollow uses the same transactional cleanup path as blocking.
    let mut conn = f.pool.get().unwrap();
    conn.batch_execute(&format!(
        "DELETE FROM relationships WHERE requester_user_id='{}' AND target_user_id='{}'",
        f.reader, f.owner
    ))
    .unwrap();
    PostGrant::revoke_all_direct_between_users_with_conn(f.owner, f.reader, &mut conn).unwrap();
    drop(conn);
    assert!(!f.can_read(past));
    assert!(!f.can_read(manual));
    assert!(
        !persist::member(&mut *f.pool.get().unwrap(), a.id, f.reader)
            .unwrap()
            .future_access_enabled
    );
    assert_eq!(
        persist::add_member(f.owner, a.id, f.reader, true, &f.pool)
            .unwrap_err()
            .status_code,
        403
    );
    persist::archive(f.owner, a.id, false, &f.pool).unwrap();
    assert!(persist::default_audiences(f.owner, f.journal, &f.pool)
        .unwrap()
        .iter()
        .all(|audience| audience.id != a.id));
}

#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn concurrent_audiences_keep_one_grant_with_multiple_sources() {
    let f = Fixture::new();
    let a = persist::create(f.owner, "A", &f.pool).unwrap();
    let b = persist::create(f.owner, "B", &f.pool).unwrap();
    persist::add_member(f.owner, a.id, f.reader, true, &f.pool).unwrap();
    persist::add_member(f.owner, b.id, f.reader, true, &f.pool).unwrap();
    let (_, post) = f.publication();
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "INSERT INTO post_audiences(post_id,audience_id)
        VALUES('{post}','{}'),('{post}','{}')",
            a.id, b.id
        ))
        .unwrap();
    #[derive(QueryableByName)]
    struct Changed {
        #[diesel(sql_type=diesel::sql_types::Bool)]
        changed: bool,
    }
    let barrier = Arc::new(Barrier::new(2));
    let handles = [a.id, b.id]
        .into_iter()
        .map(|aid| {
            let pool = f.pool.clone();
            let reader = f.reader;
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                diesel::sql_query("SELECT grant_audience_post($1,$2,$3,TRUE) AS changed")
                    .bind::<SqlUuid, _>(aid)
                    .bind::<SqlUuid, _>(post)
                    .bind::<SqlUuid, _>(reader)
                    .get_result::<Changed>(&mut pool.get().unwrap())
                    .unwrap()
                    .changed
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .filter(|changed| *changed)
            .count(),
        1
    );
    let grants = PostGrant::find_for_post_paginated(post, 0, 100, &f.pool)
        .unwrap()
        .0;
    assert_eq!(grants.len(), 1);
    assert!(matches!(grants[0].audience_id,Some(id) if id==a.id||id==b.id));
    assert_eq!(grants[0].sources.len(), 2);
    assert!(grants[0].sources.iter().all(|s| s.status == "active"));
    let removed = persist::detach(f.owner, a.id, post, true, &f.pool).unwrap();
    assert_eq!(removed.revoked_grant_count, 0);
    assert!(f.can_read(post), "Other audience still grants access");
    let manual = PostGrant::create_or_update(
        &Post::find_full(post, &f.pool).unwrap(),
        f.owner,
        NewPostGrantDto {
            grantee_user_id: Some(f.reader),
            grantee_scope: None,
            access_level: None,
        },
        &f.pool,
    )
    .unwrap();
    assert!(manual
        .sources
        .iter()
        .any(|s| s.source_type == "manual" && s.status == "active"));
    persist::detach(f.owner, b.id, post, true, &f.pool).unwrap();
    assert!(
        f.can_read(post),
        "Manual access survives removing both audiences"
    );
    let revoked = PostGrant::revoke(post, manual.id, f.owner, &f.pool).unwrap();
    assert!(revoked.sources.iter().all(|s| s.status == "revoked"));
    assert!(!f.can_read(post));
    let pool = f.pool.clone();
    let owner = f.owner;
    drop(f);
    #[derive(QueryableByName)]
    struct Exists {
        #[diesel(sql_type=diesel::sql_types::Bool)]
        exists: bool,
    }
    assert!(
        !diesel::sql_query("SELECT EXISTS(SELECT 1 FROM users WHERE id=$1) AS exists")
            .bind::<SqlUuid, _>(owner)
            .get_result::<Exists>(&mut pool.get().unwrap())
            .unwrap()
            .exists,
        "Audience-origin grants must not prevent account deletion"
    );
}
