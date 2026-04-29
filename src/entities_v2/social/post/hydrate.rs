use chrono::NaiveDateTime;
use diesel::dsl::not;
use diesel::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::post_grant::PostGrant;
use crate::entities_v2::shared::MaturingState;
use crate::schema::{journals, posts, traces, user_post_states};

use super::model::{Post, PostAudienceRole, PostInteractionType, PostStatus, PostType};

type PostTuple = (
    Uuid,
    Uuid,
    Option<Uuid>,
    String,
    String,
    String,
    Option<String>,
    Option<Uuid>,
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

type DigestVisiblePostTuple = (
    Uuid,
    Uuid,
    Option<Uuid>,
    String,
    String,
    String,
    Option<String>,
    Option<Uuid>,
    String,
    String,
    Option<NaiveDateTime>,
    String,
    String,
    String,
    String,
    NaiveDateTime,
    NaiveDateTime,
    Uuid,
);

#[derive(Debug, Clone)]
pub struct DigestVisiblePost {
    pub post: Post,
    pub journal_id: Uuid,
}

fn tuple_to_post(row: PostTuple) -> Post {
    let (
        id,
        user_id,
        source_trace_id,
        title,
        subtitle,
        content,
        image_url,
        image_asset_id,
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
        image_asset_id,
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
    posts::image_asset_id,
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
        posts::image_asset_id,
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

fn tuple_to_digest_visible_post(row: DigestVisiblePostTuple) -> DigestVisiblePost {
    let (
        id,
        user_id,
        source_trace_id,
        title,
        subtitle,
        content,
        image_url,
        image_asset_id,
        interaction_type_raw,
        post_type_raw,
        publishing_date,
        status_raw,
        audience_role_raw,
        publishing_state,
        maturing_state_raw,
        created_at,
        updated_at,
        journal_id,
    ) = row;

    DigestVisiblePost {
        post: Post {
            id,
            resource_id: id,
            source_trace_id,
            title,
            subtitle,
            content,
            image_url,
            image_asset_id,
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
        },
        journal_id,
    }
}

impl Post {
    pub fn find_feed_paginated(
        viewer_user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Post>, i64), PpdcError> {
        let mut conn = pool.get()?;
        let visible_post_ids = PostGrant::find_visible_post_ids_for_user(viewer_user_id, pool)?;

        let mut query = posts::table
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
            .load::<PostTuple>(&mut conn)?;

        let mut standalone_posts = Vec::new();
        let mut best_by_trace_id = HashMap::<Uuid, Post>::new();

        for row in rows {
            let post = tuple_to_post(row);
            if let Some(source_trace_id) = post.source_trace_id {
                let should_replace = best_by_trace_id
                    .get(&source_trace_id)
                    .map(|current| is_better_feed_candidate(&post, current))
                    .unwrap_or(true);
                if should_replace {
                    best_by_trace_id.insert(source_trace_id, post);
                }
            } else {
                standalone_posts.push(post);
            }
        }

        let mut deduplicated_posts = standalone_posts;
        deduplicated_posts.extend(best_by_trace_id.into_values());
        deduplicated_posts.sort_by(|a, b| feed_sort_key(b).cmp(&feed_sort_key(a)));

        let total = deduplicated_posts.len() as i64;
        let items = deduplicated_posts
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect::<Vec<_>>();

        Ok((items, total))
    }

    pub fn find(id: Uuid, pool: &DbPool) -> Result<Post, PpdcError> {
        let mut conn = pool.get()?;
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
        let mut conn = pool.get()?;
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

    pub fn find_public_default_for_journal_paginated(
        journal_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Post>, i64), PpdcError> {
        let mut conn = pool.get()?;

        let total = posts::table
            .inner_join(traces::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .filter(traces::journal_id.eq(journal_id))
            .filter(posts::status.eq(PostStatus::Published.to_db()))
            .filter(posts::audience_role.eq(PostAudienceRole::Default.to_db()))
            .count()
            .get_result::<i64>(&mut conn)?;

        let rows = posts::table
            .inner_join(traces::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .filter(traces::journal_id.eq(journal_id))
            .filter(posts::status.eq(PostStatus::Published.to_db()))
            .filter(posts::audience_role.eq(PostAudienceRole::Default.to_db()))
            .select(select_post_columns())
            .order(posts::publishing_date.desc().nulls_last())
            .then_order_by(posts::created_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<PostTuple>(&mut conn)?;

        Ok((rows.into_iter().map(tuple_to_post).collect(), total))
    }

    pub fn public_default_post_uses_image_asset_in_journal(
        journal_id: Uuid,
        asset_id: Uuid,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
        let mut conn = pool.get()?;

        let matching_post_id = posts::table
            .inner_join(traces::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .filter(traces::journal_id.eq(journal_id))
            .filter(posts::status.eq(PostStatus::Published.to_db()))
            .filter(posts::audience_role.eq(PostAudienceRole::Default.to_db()))
            .filter(posts::image_asset_id.eq(Some(asset_id)))
            .select(posts::id)
            .first::<Uuid>(&mut conn)
            .optional()?;

        Ok(matching_post_id.is_some())
    }

    pub fn find_visible_shared_published_for_user_in_period(
        viewer_user_id: Uuid,
        period_start: NaiveDateTime,
        period_end: NaiveDateTime,
        pool: &DbPool,
    ) -> Result<Vec<DigestVisiblePost>, PpdcError> {
        let mut conn = pool.get()?;

        let rows = posts::table
            .inner_join(traces::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .inner_join(journals::table.on(traces::journal_id.eq(journals::id)))
            .filter(posts::status.eq(PostStatus::Published.to_db()))
            .filter(posts::publishing_date.is_not_null())
            .filter(posts::publishing_date.ge(Some(period_start)))
            .filter(posts::publishing_date.lt(Some(period_end)))
            .filter(posts::user_id.ne(viewer_user_id))
            .filter(journals::is_encrypted.eq(false))
            .select((
                posts::id,
                posts::user_id,
                posts::source_trace_id,
                posts::title,
                posts::subtitle,
                posts::content,
                posts::image_url,
                posts::image_asset_id,
                posts::interaction_type,
                posts::post_type,
                posts::publishing_date,
                posts::status,
                posts::audience_role,
                posts::publishing_state,
                posts::maturing_state,
                posts::created_at,
                posts::updated_at,
                journals::id,
            ))
            .order(posts::publishing_date.desc().nulls_last())
            .then_order_by(posts::created_at.desc())
            .load::<DigestVisiblePostTuple>(&mut conn)?;

        let mut visible_posts = Vec::new();
        for row in rows {
            let visible_post = tuple_to_digest_visible_post(row);
            if PostGrant::user_can_read_post(&visible_post.post, viewer_user_id, pool)? {
                visible_posts.push(visible_post);
            }
        }

        Ok(visible_posts)
    }

    pub fn find_for_trace_paginated(
        trace_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Post>, i64), PpdcError> {
        let mut conn = pool.get()?;

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

    pub fn find_default_for_trace(
        trace_id: Uuid,
        pool: &DbPool,
    ) -> Result<Option<Post>, PpdcError> {
        let mut conn = pool.get()?;

        let row = posts::table
            .filter(posts::source_trace_id.eq(Some(trace_id)))
            .filter(posts::audience_role.eq(PostAudienceRole::Default.to_db()))
            .select(select_post_columns())
            .order(posts::created_at.desc())
            .first::<PostTuple>(&mut conn)
            .optional()?;

        Ok(row.map(tuple_to_post))
    }

    pub fn find_for_user(
        viewer_user_id: Uuid,
        user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<Vec<Post>, PpdcError> {
        let (items, _) = Self::find_for_user_filtered_paginated(
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
        let mut conn = pool.get()?;
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
        journal_id: Option<Uuid>,
        status: Option<PostStatus>,
        seen: Option<bool>,
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
            journal_id,
            status,
            seen,
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
        journal_id: Option<Uuid>,
        status: Option<PostStatus>,
        seen: Option<bool>,
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

        let mut conn = pool.get()?;
        let visible_post_ids = PostGrant::find_shared_post_ids_for_user(viewer_user_id, pool)?;
        let seen_post_ids = if seen.is_some() {
            user_post_states::table
                .filter(user_post_states::user_id.eq(viewer_user_id))
                .select(user_post_states::post_id)
                .load::<Uuid>(&mut conn)?
        } else {
            vec![]
        };

        let mut count_query = posts::table
            .left_join(traces::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .into_boxed();
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
        if let Some(journal_id) = journal_id {
            count_query = count_query.filter(traces::journal_id.eq(journal_id));
        }
        if let Some(status) = status {
            count_query = count_query.filter(posts::status.eq(status.to_db()));
        }
        if let Some(seen) = seen {
            if seen {
                if seen_post_ids.is_empty() {
                    count_query = count_query.filter(posts::user_id.eq(viewer_user_id));
                } else {
                    count_query = count_query.filter(
                        posts::user_id
                            .eq(viewer_user_id)
                            .or(posts::id.eq_any(seen_post_ids.clone())),
                    );
                }
            } else {
                count_query = count_query.filter(posts::user_id.ne(viewer_user_id));
                if !seen_post_ids.is_empty() {
                    count_query = count_query.filter(not(posts::id.eq_any(seen_post_ids.clone())));
                }
            }
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

        let mut query = posts::table
            .left_join(traces::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .into_boxed();
        if !interaction_type_values.is_empty() {
            query = query.filter(posts::interaction_type.eq_any(interaction_type_values));
        }
        if !effective_post_types.is_empty() {
            query = query.filter(posts::post_type.eq_any(post_type_values));
        }
        if let Some(user_id) = user_id {
            query = query.filter(posts::user_id.eq(user_id));
        }
        if let Some(journal_id) = journal_id {
            query = query.filter(traces::journal_id.eq(journal_id));
        }
        if let Some(status) = status {
            query = query.filter(posts::status.eq(status.to_db()));
        }
        if let Some(seen) = seen {
            if seen {
                if seen_post_ids.is_empty() {
                    query = query.filter(posts::user_id.eq(viewer_user_id));
                } else {
                    query = query.filter(
                        posts::user_id
                            .eq(viewer_user_id)
                            .or(posts::id.eq_any(seen_post_ids)),
                    );
                }
            } else {
                query = query.filter(posts::user_id.ne(viewer_user_id));
                if !seen_post_ids.is_empty() {
                    query = query.filter(not(posts::id.eq_any(seen_post_ids)));
                }
            }
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

    pub fn find_drafts_for_user_paginated(
        user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Post>, i64), PpdcError> {
        let mut conn = pool.get()?;

        let total = posts::table
            .filter(posts::user_id.eq(user_id))
            .filter(posts::status.eq(PostStatus::Draft.to_db()))
            .count()
            .get_result::<i64>(&mut conn)?;

        let rows = posts::table
            .filter(posts::user_id.eq(user_id))
            .filter(posts::status.eq(PostStatus::Draft.to_db()))
            .select(select_post_columns())
            .order(posts::updated_at.desc())
            .then_order_by(posts::created_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<PostTuple>(&mut conn)?;

        Ok((rows.into_iter().map(tuple_to_post).collect(), total))
    }
}

fn feed_published_or_created_at(post: &Post) -> NaiveDateTime {
    post.publishing_date.unwrap_or(post.created_at)
}

fn audience_priority_for_feed(post: &Post) -> i32 {
    match post.audience_role {
        PostAudienceRole::Restricted => 1,
        PostAudienceRole::Default => 0,
    }
}

fn is_better_feed_candidate(candidate: &Post, current: &Post) -> bool {
    let candidate_priority = audience_priority_for_feed(candidate);
    let current_priority = audience_priority_for_feed(current);
    if candidate_priority != current_priority {
        return candidate_priority > current_priority;
    }

    let candidate_published_at = feed_published_or_created_at(candidate);
    let current_published_at = feed_published_or_created_at(current);
    if candidate_published_at != current_published_at {
        return candidate_published_at > current_published_at;
    }

    candidate.created_at > current.created_at
}

fn feed_sort_key(post: &Post) -> (NaiveDateTime, NaiveDateTime, Uuid) {
    (feed_published_or_created_at(post), post.created_at, post.id)
}
