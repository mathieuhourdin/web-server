use chrono::NaiveDateTime;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::shared::MaturingState;
use crate::schema::posts;

use super::model::{Post, PostInteractionType, PostType};

type PostTuple = (
    Uuid,
    Uuid,
    String,
    String,
    String,
    Option<String>,
    String,
    String,
    Option<NaiveDateTime>,
    String,
    String,
    NaiveDateTime,
    NaiveDateTime,
);

fn tuple_to_post(row: PostTuple) -> Post {
    let (
        id,
        user_id,
        title,
        subtitle,
        content,
        image_url,
        interaction_type_raw,
        post_type_raw,
        publishing_date,
        publishing_state,
        maturing_state_raw,
        created_at,
        updated_at,
    ) = row;

    Post {
        id,
        resource_id: id,
        title,
        subtitle,
        content,
        image_url,
        interaction_type: PostInteractionType::from_db(&interaction_type_raw),
        post_type: PostType::from_db(&post_type_raw),
        user_id,
        publishing_date,
        publishing_state,
        maturing_state: MaturingState::from_code(&maturing_state_raw)
            .unwrap_or(MaturingState::Draft),
        created_at,
        updated_at,
    }
}

fn select_post_columns() -> (
    posts::id,
    posts::user_id,
    posts::title,
    posts::subtitle,
    posts::content,
    posts::image_url,
    posts::interaction_type,
    posts::post_type,
    posts::publishing_date,
    posts::publishing_state,
    posts::maturing_state,
    posts::created_at,
    posts::updated_at,
) {
    (
        posts::id,
        posts::user_id,
        posts::title,
        posts::subtitle,
        posts::content,
        posts::image_url,
        posts::interaction_type,
        posts::post_type,
        posts::publishing_date,
        posts::publishing_state,
        posts::maturing_state,
        posts::created_at,
        posts::updated_at,
    )
}

impl Post {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Post, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let row = posts::table
            .filter(posts::id.eq(id))
            .select(select_post_columns())
            .first::<PostTuple>(&mut conn)
            .optional()?;
        row.map(tuple_to_post)
            .ok_or_else(|| PpdcError::new(404, ErrorType::ApiError, "Post not found".to_string()))
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
        Self::find_for_user_filtered(user_id, vec![], vec![], offset, limit, pool)
    }

    pub fn find_for_user_filtered(
        user_id: Uuid,
        interaction_types: Vec<PostInteractionType>,
        post_types: Vec<PostType>,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<Vec<Post>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let mut query = posts::table
            .filter(posts::user_id.eq(user_id))
            .into_boxed();

        if !interaction_types.is_empty() {
            let interaction_type_values = interaction_types
                .into_iter()
                .map(|value| value.to_db())
                .collect::<Vec<_>>();
            query = query.filter(posts::interaction_type.eq_any(interaction_type_values));
        }
        if !post_types.is_empty() {
            let post_type_values = post_types
                .into_iter()
                .map(|value| value.to_db())
                .collect::<Vec<_>>();
            query = query.filter(posts::post_type.eq_any(post_type_values));
        }

        let rows = query
            .select(select_post_columns())
            .order(posts::publishing_date.desc().nulls_last())
            .then_order_by(posts::created_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<PostTuple>(&mut conn)?;
        Ok(rows.into_iter().map(tuple_to_post).collect())
    }

    pub fn find_filtered(
        interaction_types: Vec<PostInteractionType>,
        post_types: Vec<PostType>,
        legacy_resource_type: Option<String>,
        _is_external: Option<bool>,
        user_id: Option<Uuid>,
        maturing_state: Option<MaturingState>,
        limit: i64,
        pool: &DbPool,
    ) -> Result<Vec<Post>, PpdcError> {
        let mapped_post_type = legacy_resource_type.as_deref().and_then(|value| {
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
        if !interaction_types.is_empty() {
            let interaction_type_values = interaction_types
                .into_iter()
                .map(|value| value.to_db())
                .collect::<Vec<_>>();
            query = query.filter(posts::interaction_type.eq_any(interaction_type_values));
        }
        let effective_post_types = if post_types.is_empty() {
            mapped_post_type.into_iter().collect::<Vec<_>>()
        } else {
            post_types
        };
        if !effective_post_types.is_empty() {
            let post_type_values = effective_post_types
                .into_iter()
                .map(|value| value.to_db())
                .collect::<Vec<_>>();
            query = query.filter(posts::post_type.eq_any(post_type_values));
        }
        if let Some(user_id) = user_id {
            query = query.filter(posts::user_id.eq(user_id));
        }
        if let Some(maturing_state) = maturing_state {
            let maturing_state_code = maturing_state.to_code().to_string();
            query = query.filter(posts::maturing_state.eq(maturing_state_code));
        }

        let rows = query
            .select(select_post_columns())
            .order(posts::publishing_date.desc().nulls_last())
            .then_order_by(posts::created_at.desc())
            .limit(limit.max(1))
            .load::<PostTuple>(&mut conn)?;

        Ok(rows.into_iter().map(tuple_to_post).collect())
    }
}
