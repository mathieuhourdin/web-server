use diesel::prelude::*;
use diesel::sql_types::Uuid as SqlUuid;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::schema::posts;

use super::model::{NewPost, Post, PostSourceRef};

impl Post {
    pub fn update(self, pool: &DbPool) -> Result<Post, PpdcError> {
        let mut conn = pool.get()?;
        diesel::update(posts::table.filter(posts::id.eq(self.id)))
            .set((
                posts::source_trace_id.eq(self.source_trace_id),
                posts::source_document_id.eq(self.source_document_id),
                posts::source_album_id.eq(self.source_album_id),
                posts::interaction_type.eq(self.interaction_type.to_db()),
                posts::post_type.eq(self.post_type.to_db()),
                posts::publishing_date.eq(self.publishing_date),
                posts::status.eq(self.status.to_db()),
                posts::audience_role.eq(self.audience_role.to_db()),
                posts::user_id.eq(self.user_id),
            ))
            .execute(&mut conn)?;
        Post::find_full(self.id, pool)
    }
}

impl NewPost {
    pub fn create(self, pool: &DbPool) -> Result<Post, PpdcError> {
        let mut conn = pool.get()?;
        let id: Uuid = diesel::insert_into(posts::table)
            .values((
                posts::source_trace_id.eq(self.source_trace_id),
                posts::source_document_id.eq(self.source_document_id),
                posts::source_album_id.eq(self.source_album_id),
                posts::title.eq(""),
                posts::subtitle.eq(""),
                posts::content.eq(""),
                posts::interaction_type.eq(self.interaction_type.to_db()),
                posts::post_type.eq(self.post_type.to_db()),
                posts::user_id.eq(self.user_id),
                posts::publishing_date.eq(self.publishing_date),
                posts::status.eq(self.status.to_db()),
                posts::audience_role.eq(self.audience_role.to_db()),
            ))
            .returning(posts::id)
            .get_result(&mut conn)?;
        Post::find_full(id, pool)
    }
}

/// Enforces the one-directional publication invariant from `doc/publication.md`:
/// when a source record's status no longer permits a published post (e.g. it has
/// been archived), its related post is archived too, so that owner access never
/// outranks follower access. It never auto-publishes — publishing stays a
/// deliberate user action — so eligible sources leave their post untouched.
///
/// Callers pass `permits_published_post` computed from their own status enum
/// (see `*Status::permits_published_post`). The cascade is idempotent and must
/// run inside the caller's transaction so it commits atomically with the source
/// status change. Post↔source is 1:1, so at most one post is affected.
pub fn enforce_publication_invariant_for_source(
    source: PostSourceRef,
    permits_published_post: bool,
    conn: &mut PgConnection,
) -> Result<(), diesel::result::Error> {
    if permits_published_post {
        return Ok(());
    }

    let (sql, source_id) = match source {
        PostSourceRef::Trace(id) => (
            "UPDATE posts SET status = 'ARCHIVED', updated_at = NOW() \
             WHERE source_trace_id = $1 AND status <> 'ARCHIVED'",
            id,
        ),
        PostSourceRef::Document(id) => (
            "UPDATE posts SET status = 'ARCHIVED', updated_at = NOW() \
             WHERE source_document_id = $1 AND status <> 'ARCHIVED'",
            id,
        ),
        PostSourceRef::Album(id) => (
            "UPDATE posts SET status = 'ARCHIVED', updated_at = NOW() \
             WHERE source_album_id = $1 AND status <> 'ARCHIVED'",
            id,
        ),
    };

    diesel::sql_query(sql)
        .bind::<SqlUuid, _>(source_id)
        .execute(conn)?;
    Ok(())
}

/// Guards the publish direction of the publication invariant: a post may only be
/// set to published when its source record currently permits it (see
/// `*Status::permits_published_post`). This complements
/// `enforce_publication_invariant_for_source`, which handles the inverse
/// (source-mutation) direction. Callers pass the source's eligibility and invoke
/// this before persisting a post that would become published.
pub fn ensure_source_permits_published_post(permits_published_post: bool) -> Result<(), PpdcError> {
    if permits_published_post {
        return Ok(());
    }
    Err(PpdcError::new(
        400,
        ErrorType::ApiError,
        "The linked record is not in a publishable state".to_string(),
    ))
}
