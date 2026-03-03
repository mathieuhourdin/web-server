use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::schema::posts;

use super::model::{NewPost, Post};

impl Post {
    pub fn update(self, pool: &DbPool) -> Result<Post, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        diesel::update(posts::table.filter(posts::id.eq(self.id)))
            .set((
                posts::title.eq(self.title),
                posts::subtitle.eq(self.subtitle),
                posts::content.eq(self.content),
                posts::image_url.eq(self.image_url),
                posts::interaction_type.eq(self.interaction_type.to_db()),
                posts::post_type.eq(self.post_type.to_db()),
                posts::publishing_date.eq(self.publishing_date),
                posts::publishing_state.eq(self.publishing_state),
                posts::maturing_state.eq(self.maturing_state.to_code()),
                posts::user_id.eq(self.user_id),
            ))
            .execute(&mut conn)?;
        Post::find_full(self.id, pool)
    }
}

impl NewPost {
    pub fn create(self, pool: &DbPool) -> Result<Post, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let id: Uuid = diesel::insert_into(posts::table)
            .values((
                posts::title.eq(self.title),
                posts::subtitle.eq(self.subtitle),
                posts::content.eq(self.content),
                posts::image_url.eq(self.image_url),
                posts::interaction_type.eq(self.interaction_type.to_db()),
                posts::post_type.eq(self.post_type.to_db()),
                posts::user_id.eq(self.user_id),
                posts::publishing_date.eq(self.publishing_date),
                posts::publishing_state.eq(self.publishing_state),
                posts::maturing_state.eq(self.maturing_state.to_code()),
            ))
            .returning(posts::id)
            .get_result(&mut conn)?;
        Post::find_full(id, pool)
    }
}
