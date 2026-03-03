use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Nullable, Text, Timestamp, Uuid as SqlUuid};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::{ErrorType, PpdcError};
use crate::entities::resource::maturing_state::MaturingState;
use crate::schema::posts;

use super::model::{Post, PostType};

#[derive(QueryableByName)]
struct PostRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    user_id: Uuid,
    #[diesel(sql_type = Text)]
    title: String,
    #[diesel(sql_type = Text)]
    subtitle: String,
    #[diesel(sql_type = Text)]
    content: String,
    #[diesel(sql_type = Nullable<Text>)]
    image_url: Option<String>,
    #[diesel(sql_type = Text)]
    post_type: String,
    #[diesel(sql_type = Nullable<Timestamp>)]
    publishing_date: Option<NaiveDateTime>,
    #[diesel(sql_type = Text)]
    publishing_state: String,
    #[diesel(sql_type = Text)]
    maturing_state: String,
    #[diesel(sql_type = Timestamp)]
    created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    updated_at: NaiveDateTime,
}

fn row_to_post(row: PostRow) -> Post {
    Post {
        id: row.id,
        resource_id: row.id,
        title: row.title,
        subtitle: row.subtitle,
        content: row.content,
        image_url: row.image_url,
        post_type: PostType::from_db(&row.post_type),
        user_id: row.user_id,
        publishing_date: row.publishing_date,
        publishing_state: row.publishing_state,
        maturing_state: MaturingState::from_code(&row.maturing_state).unwrap_or(MaturingState::Draft),
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

impl Post {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Post, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let mut rows = sql_query(
            r#"
            SELECT id, user_id, title, subtitle, content, image_url, post_type, publishing_date,
                   publishing_state, maturing_state, created_at, updated_at
            FROM posts
            WHERE id = $1
            "#,
        )
        .bind::<SqlUuid, _>(id)
        .load::<PostRow>(&mut conn)?;
        rows.pop().map(row_to_post).ok_or_else(|| {
            PpdcError::new(404, ErrorType::ApiError, "Post not found".to_string())
        })
    }

    pub fn find_full(id: Uuid, pool: &DbPool) -> Result<Post, PpdcError> {
        Post::find(id, pool)
    }

    pub fn find_for_user(
        user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<Vec<Post>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let rows = posts::table
            .filter(posts::user_id.eq(user_id))
            .order(posts::publishing_date.desc().nulls_last())
            .then_order_by(posts::created_at.desc())
            .offset(offset)
            .limit(limit)
            .select(posts::id)
            .load::<Uuid>(&mut conn)?;
        rows.into_iter().map(|id| Post::find(id, pool)).collect()
    }

    pub fn find_filtered(
        post_type: Option<PostType>,
        resource_type: Option<String>,
        _is_external: Option<bool>,
        user_id: Option<Uuid>,
        maturing_state: Option<MaturingState>,
        limit: i64,
        pool: &DbPool,
    ) -> Result<Vec<Post>, PpdcError> {
        let mapped_resource_type = resource_type
            .as_deref()
            .and_then(|value| {
                if value == "all" {
                    None
                } else {
                    Some(PostType::from_code(value))
                }
            });

        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let mut query = posts::table.into_boxed();
        if let Some(post_type) = post_type.or(mapped_resource_type) {
            query = query.filter(posts::post_type.eq(post_type.to_db()));
        }
        if let Some(user_id) = user_id {
            query = query.filter(posts::user_id.eq(user_id));
        }
        if let Some(maturing_state) = maturing_state {
            let maturing_state_code = maturing_state.to_code().to_string();
            query = query.filter(posts::maturing_state.eq(maturing_state_code));
        }

        let ids = query
            .order(posts::publishing_date.desc().nulls_last())
            .then_order_by(posts::created_at.desc())
            .limit(limit.max(1))
            .select(posts::id)
            .load::<Uuid>(&mut conn)?;

        ids.into_iter().map(|id| Post::find(id, pool)).collect()
    }
}
