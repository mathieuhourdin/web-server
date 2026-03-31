use chrono::NaiveDateTime;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::post_grant::PostGrant;
use crate::entities_v2::shared::MaturingState;
use crate::schema::{posts, traces};

use super::model::{Post, PostAudienceRole, PostInteractionType, PostStatus, PostType};

type PostTuple = (
    Uuid,
    Uuid,
    Option<Uuid>,
    String,
    String,
    String,
    Option<String>,
    String,
    String,
    Option<NaiveDateTime>,
    String,
    String,
    String,
    String,
    NaiveDateTime,
    NaiveDateTime,
);

fn tuple_to_post(row: PostTuple) -> Post {
    let (
        id,
        user_id,
        source_trace_id,
        title,
        subtitle,
        content,
        image_url,
        interaction_type_raw,
        post_type_raw,
        publishing_date,
        status_raw,
        audience_role_raw,
        publishing_state,
        maturing_state_raw,
        created_at,
        updated_at,
    ) = row;

    Post {
        id,
        resource_id: id,
        source_trace_id,
        title,
        subtitle,
        content,
        image_url,
        interaction_type: PostInteractionType::from_db(&interaction_type_raw),
        post_type: PostType::from_db(&post_type_raw),
        user_id,
        publishing_date,
        status: PostStatus::from_db(&status_raw),
        audience_role: PostAudienceRole::from_db(&audience_role_raw),
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
    posts::source_trace_id,
    posts::title,
    posts::subtitle,
    posts::content,
    posts::image_url,
    posts::interaction_type,
    posts::post_type,
    posts::publishing_date,
    posts::status,
    posts::audience_role,
    posts::publishing_state,
    posts::maturing_state,
    posts::created_at,
    posts::updated_at,
) {
    (
        posts::id,
        posts::user_id,
        posts::source_trace_id,
        posts::title,
        posts::subtitle,
        posts::content,
        posts::image_url,
        posts::interaction_type,
        posts::post_type,
        posts::publishing_date,
        posts::status,
        posts::audience_role,
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

    pub fn find_for_journal_paginated(
        viewer_user_id: Uuid,
        journal_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Post>, i64), PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let visible_post_ids = PostGrant::find_shared_post_ids_for_user(viewer_user_id, pool)?;

        let mut count_query = posts::table
            .inner_join(traces::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .filter(traces::journal_id.eq(journal_id))
            .filter(posts::status.eq(PostStatus::Published.to_db()))
            .into_boxed();

        if visible_post_ids.is_empty() {
            count_query = count_query.filter(posts::user_id.eq(viewer_user_id));
        } else {
            count_query = count_query.filter(
                posts::user_id
                    .eq(viewer_user_id)
                    .or(posts::id.eq_any(visible_post_ids.clone())),
            );
        }

        let total = count_query.count().get_result::<i64>(&mut conn)?;

        let mut query = posts::table
            .inner_join(traces::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .filter(traces::journal_id.eq(journal_id))
            .filter(posts::status.eq(PostStatus::Published.to_db()))
            .into_boxed();

        if visible_post_ids.is_empty() {
            query = query.filter(posts::user_id.eq(viewer_user_id));
        } else {
            query = query.filter(
                posts::user_id
                    .eq(viewer_user_id)
                    .or(posts::id.eq_any(visible_post_ids)),
            );
        }

        let rows = query
            .select(select_post_columns())
            .order(posts::publishing_date.desc().nulls_last())
            .then_order_by(posts::created_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<PostTuple>(&mut conn)?;

        Ok((rows.into_iter().map(tuple_to_post).collect(), total))
    }

    pub fn find_for_trace_paginated(
        trace_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Post>, i64), PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let total = posts::table
            .filter(posts::source_trace_id.eq(Some(trace_id)))
            .count()
            .get_result::<i64>(&mut conn)?;

        let rows = posts::table
            .filter(posts::source_trace_id.eq(Some(trace_id)))
            .select(select_post_columns())
            .order(posts::publishing_date.desc().nulls_last())
            .then_order_by(posts::created_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<PostTuple>(&mut conn)?;

        Ok((rows.into_iter().map(tuple_to_post).collect(), total))
    }

    pub fn find_for_user(
        viewer_user_id: Uuid,
        user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<Vec<Post>, PpdcError> {
        let (items, _) =
            Self::find_for_user_filtered_paginated(
                viewer_user_id,
                user_id,
                vec![],
                vec![],
                offset,
                limit,
                pool,
            )?;
        Ok(items)
    }

    pub fn find_for_user_filtered(
        viewer_user_id: Uuid,
        user_id: Uuid,
        interaction_types: Vec<PostInteractionType>,
        post_types: Vec<PostType>,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<Vec<Post>, PpdcError> {
        let (items, _) = Self::find_for_user_filtered_paginated(
            viewer_user_id,
            user_id,
            interaction_types,
            post_types,
            offset,
            limit,
            pool,
        )?;
        Ok(items)
    }

    pub fn find_for_user_filtered_paginated(
        viewer_user_id: Uuid,
        user_id: Uuid,
        interaction_types: Vec<PostInteractionType>,
        post_types: Vec<PostType>,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Post>, i64), PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let visible_post_ids = if viewer_user_id == user_id {
            vec![]
        } else {
            PostGrant::find_shared_post_ids_for_user(viewer_user_id, pool)?
        };
        let mut count_query = posts::table.filter(posts::user_id.eq(user_id)).into_boxed();
        let interaction_type_values = interaction_types
            .iter()
            .map(|value| value.to_db())
            .collect::<Vec<_>>();
        let post_type_values = post_types
            .iter()
            .map(|value| value.to_db())
            .collect::<Vec<_>>();

        if !interaction_type_values.is_empty() {
            count_query =
                count_query.filter(posts::interaction_type.eq_any(interaction_type_values.clone()));
        }
        if !post_type_values.is_empty() {
            count_query = count_query.filter(posts::post_type.eq_any(post_type_values.clone()));
        }
        count_query = count_query.filter(posts::status.eq(PostStatus::Published.to_db()));
        if viewer_user_id != user_id {
            if visible_post_ids.is_empty() {
                return Ok((vec![], 0));
            }
            count_query = count_query.filter(posts::id.eq_any(visible_post_ids.clone()));
        }

        let total = count_query.count().get_result::<i64>(&mut conn)?;

        let mut query = posts::table.filter(posts::user_id.eq(user_id)).into_boxed();

        if !interaction_type_values.is_empty() {
            query = query.filter(posts::interaction_type.eq_any(interaction_type_values));
        }
        if !post_type_values.is_empty() {
            query = query.filter(posts::post_type.eq_any(post_type_values));
        }
        query = query.filter(posts::status.eq(PostStatus::Published.to_db()));
        if viewer_user_id != user_id {
            query = query.filter(posts::id.eq_any(visible_post_ids));
        }

        let rows = query
            .select(select_post_columns())
            .order(posts::publishing_date.desc().nulls_last())
            .then_order_by(posts::created_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<PostTuple>(&mut conn)?;
        Ok((rows.into_iter().map(tuple_to_post).collect(), total))
    }

    pub fn find_filtered(
        viewer_user_id: Uuid,
        interaction_types: Vec<PostInteractionType>,
        post_types: Vec<PostType>,
        legacy_resource_type: Option<String>,
        _is_external: Option<bool>,
        user_id: Option<Uuid>,
        status: Option<PostStatus>,
        limit: i64,
        pool: &DbPool,
    ) -> Result<Vec<Post>, PpdcError> {
        let (items, _) = Self::find_filtered_paginated(
            viewer_user_id,
            interaction_types,
            post_types,
            legacy_resource_type,
            _is_external,
            user_id,
            status,
            0,
            limit,
            pool,
        )?;
        Ok(items)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn find_filtered_paginated(
        viewer_user_id: Uuid,
        interaction_types: Vec<PostInteractionType>,
        post_types: Vec<PostType>,
        legacy_resource_type: Option<String>,
        _is_external: Option<bool>,
        user_id: Option<Uuid>,
        status: Option<PostStatus>,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Post>, i64), PpdcError> {
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
        let visible_post_ids = PostGrant::find_shared_post_ids_for_user(viewer_user_id, pool)?;

        let mut count_query = posts::table.into_boxed();
        let effective_post_types = if post_types.is_empty() {
            mapped_post_type.into_iter().collect::<Vec<_>>()
        } else {
            post_types
        };
        let interaction_type_values = interaction_types
            .iter()
            .map(|value| value.to_db())
            .collect::<Vec<_>>();
        let post_type_values = effective_post_types
            .iter()
            .map(|value| value.to_db())
            .collect::<Vec<_>>();
        if !interaction_type_values.is_empty() {
            count_query =
                count_query.filter(posts::interaction_type.eq_any(interaction_type_values.clone()));
        }
        if !post_type_values.is_empty() {
            count_query = count_query.filter(posts::post_type.eq_any(post_type_values.clone()));
        }
        if let Some(user_id) = user_id {
            count_query = count_query.filter(posts::user_id.eq(user_id));
        }
        if let Some(status) = status {
            count_query = count_query.filter(posts::status.eq(status.to_db()));
        }
        if visible_post_ids.is_empty() {
            count_query = count_query.filter(posts::user_id.eq(viewer_user_id));
        } else {
            count_query = count_query.filter(
                posts::user_id
                    .eq(viewer_user_id)
                    .or(posts::id.eq_any(visible_post_ids.clone())),
            );
        }

        let total = count_query.count().get_result::<i64>(&mut conn)?;

        let mut query = posts::table.into_boxed();
        if !interaction_type_values.is_empty() {
            query = query.filter(posts::interaction_type.eq_any(interaction_type_values));
        }
        if !effective_post_types.is_empty() {
            query = query.filter(posts::post_type.eq_any(post_type_values));
        }
        if let Some(user_id) = user_id {
            query = query.filter(posts::user_id.eq(user_id));
        }
        if let Some(status) = status {
            query = query.filter(posts::status.eq(status.to_db()));
        }
        if visible_post_ids.is_empty() {
            query = query.filter(posts::user_id.eq(viewer_user_id));
        } else {
            query = query.filter(
                posts::user_id
                    .eq(viewer_user_id)
                    .or(posts::id.eq_any(visible_post_ids)),
            );
        }

        let rows = query
            .select(select_post_columns())
            .order(posts::publishing_date.desc().nulls_last())
            .then_order_by(posts::created_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<PostTuple>(&mut conn)?;

        Ok((rows.into_iter().map(tuple_to_post).collect(), total))
    }
}
