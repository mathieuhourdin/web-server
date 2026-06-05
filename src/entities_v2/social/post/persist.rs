use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::schema::posts;

use super::model::{NewPost, Post};

impl Post {
    pub fn update(self, pool: &DbPool) -> Result<Post, PpdcError> {
        let mut conn = pool.get()?;
        diesel::update(posts::table.filter(posts::id.eq(self.id)))
            .set((
                posts::source_trace_id.eq(self.source_trace_id),
                posts::source_document_id.eq(self.source_document_id),
                posts::source_album_id.eq(self.source_album_id),
                posts::trace_version_id.eq(self.trace_version_id),
                posts::title.eq(self.title),
                posts::subtitle.eq(self.subtitle),
                posts::content.eq(self.content),
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
                posts::trace_version_id.eq(self.trace_version_id),
                posts::title.eq(self.title),
                posts::subtitle.eq(self.subtitle),
                posts::content.eq(self.content),
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
