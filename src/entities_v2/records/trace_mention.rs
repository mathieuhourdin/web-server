use std::collections::{HashMap, HashSet};

use chrono::NaiveDateTime;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    relationship::{Relationship, RelationshipStatus},
    user::{User, UserPrincipalType},
    user_block::UserBlock,
};
use crate::schema::trace_mentions;

const MAX_MENTIONS_PER_TRACE: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceMentionUser {
    pub user_id: Uuid,
    pub handle: String,
    pub display_name: String,
    pub profile_picture_display_url: Option<String>,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = trace_mentions)]
pub struct TraceMention {
    pub trace_id: Uuid,
    pub mentioned_user_id: Uuid,
    pub notified_at: Option<NaiveDateTime>,
    pub removed_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl TraceMention {
    pub fn validate_targets(
        owner_user_id: Uuid,
        mentioned_user_ids: &[Uuid],
        pool: &DbPool,
    ) -> Result<Vec<Uuid>, PpdcError> {
        let unique_ids = mentioned_user_ids.iter().copied().collect::<HashSet<_>>();
        if unique_ids.len() != mentioned_user_ids.len() {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "mentioned_user_ids cannot contain duplicates".to_string(),
            ));
        }
        if unique_ids.len() > MAX_MENTIONS_PER_TRACE {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("A trace can mention at most {MAX_MENTIONS_PER_TRACE} users"),
            ));
        }

        let mut validated = Vec::with_capacity(unique_ids.len());
        for mentioned_user_id in mentioned_user_ids {
            if *mentioned_user_id == owner_user_id {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "A user cannot mention themselves".to_string(),
                ));
            }

            let mentioned_user = User::find(mentioned_user_id, pool)?;
            if mentioned_user.principal_type != UserPrincipalType::Human
                || UserBlock::exists_in_either_direction(owner_user_id, *mentioned_user_id, pool)?
            {
                return Err(invalid_mention_target(*mentioned_user_id));
            }

            let relationship =
                Relationship::find_follow_summary_between(owner_user_id, *mentioned_user_id, pool)?;
            let accepted = relationship.viewer_to_user == Some(RelationshipStatus::Accepted)
                || relationship.user_to_viewer == Some(RelationshipStatus::Accepted);
            if !accepted {
                return Err(invalid_mention_target(*mentioned_user_id));
            }
            validated.push(*mentioned_user_id);
        }

        Ok(validated)
    }

    pub(crate) fn replace_active_with_conn(
        trace_id: Uuid,
        mentioned_user_ids: &[Uuid],
        conn: &mut PgConnection,
    ) -> Result<(), diesel::result::Error> {
        diesel::update(
            trace_mentions::table
                .filter(trace_mentions::trace_id.eq(trace_id))
                .filter(trace_mentions::removed_at.is_null())
                .filter(trace_mentions::mentioned_user_id.ne_all(mentioned_user_ids)),
        )
        .set((
            trace_mentions::removed_at.eq(diesel::dsl::now.nullable()),
            trace_mentions::updated_at.eq(diesel::dsl::now),
        ))
        .execute(conn)?;

        for mentioned_user_id in mentioned_user_ids {
            diesel::insert_into(trace_mentions::table)
                .values((
                    trace_mentions::trace_id.eq(trace_id),
                    trace_mentions::mentioned_user_id.eq(*mentioned_user_id),
                ))
                .on_conflict((trace_mentions::trace_id, trace_mentions::mentioned_user_id))
                .do_update()
                .set((
                    trace_mentions::removed_at.eq::<Option<NaiveDateTime>>(None),
                    trace_mentions::updated_at.eq(diesel::dsl::now),
                ))
                .execute(conn)?;
        }
        Ok(())
    }

    pub fn replace_active(
        trace_id: Uuid,
        mentioned_user_ids: &[Uuid],
        pool: &DbPool,
    ) -> Result<(), PpdcError> {
        let mut conn = pool.get()?;
        conn.transaction::<(), diesel::result::Error, _>(|conn| {
            Self::replace_active_with_conn(trace_id, mentioned_user_ids, conn)
        })?;
        Ok(())
    }

    pub fn find_active_user_ids_for_trace(
        trace_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Uuid>, PpdcError> {
        let mut conn = pool.get()?;
        Ok(trace_mentions::table
            .filter(trace_mentions::trace_id.eq(trace_id))
            .filter(trace_mentions::removed_at.is_null())
            .select(trace_mentions::mentioned_user_id)
            .load::<Uuid>(&mut conn)?)
    }

    pub fn find_unnotified_active_user_ids_for_trace(
        trace_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Uuid>, PpdcError> {
        let mut conn = pool.get()?;
        Ok(trace_mentions::table
            .filter(trace_mentions::trace_id.eq(trace_id))
            .filter(trace_mentions::removed_at.is_null())
            .filter(trace_mentions::notified_at.is_null())
            .select(trace_mentions::mentioned_user_id)
            .load::<Uuid>(&mut conn)?)
    }

    pub fn find_active_trace_ids_for_user(
        mentioned_user_id: Uuid,
        trace_ids: &[Uuid],
        pool: &DbPool,
    ) -> Result<HashSet<Uuid>, PpdcError> {
        if trace_ids.is_empty() {
            return Ok(HashSet::new());
        }
        let mut conn = pool.get()?;
        Ok(trace_mentions::table
            .filter(trace_mentions::trace_id.eq_any(trace_ids))
            .filter(trace_mentions::mentioned_user_id.eq(mentioned_user_id))
            .filter(trace_mentions::removed_at.is_null())
            .select(trace_mentions::trace_id)
            .load::<Uuid>(&mut conn)?
            .into_iter()
            .collect())
    }

    pub fn find_active_users_for_trace(
        trace_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<TraceMentionUser>, PpdcError> {
        let ids = Self::find_active_user_ids_for_trace(trace_id, pool)?;
        let mut users = User::find_many(&ids, pool)?;
        users.sort_by_key(|user| {
            ids.iter()
                .position(|mentioned_user_id| mentioned_user_id == &user.id)
                .unwrap_or(usize::MAX)
        });
        Ok(users
            .into_iter()
            .map(|user| TraceMentionUser {
                user_id: user.id,
                handle: user.handle.clone(),
                display_name: user.display_name(),
                profile_picture_display_url: user.profile_picture_display_url(pool),
            })
            .collect())
    }

    pub fn find_active_users_by_trace_ids(
        trace_ids: &[Uuid],
        pool: &DbPool,
    ) -> Result<HashMap<Uuid, Vec<TraceMentionUser>>, PpdcError> {
        if trace_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut conn = pool.get()?;
        let rows = trace_mentions::table
            .filter(trace_mentions::trace_id.eq_any(trace_ids))
            .filter(trace_mentions::removed_at.is_null())
            .select((
                trace_mentions::trace_id,
                trace_mentions::mentioned_user_id,
                trace_mentions::created_at,
            ))
            .order((trace_mentions::trace_id, trace_mentions::created_at))
            .load::<(Uuid, Uuid, NaiveDateTime)>(&mut conn)?;
        drop(conn);

        let user_ids = rows
            .iter()
            .map(|(_, user_id, _)| *user_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let summaries_by_user_id = User::find_many(&user_ids, pool)?
            .into_iter()
            .map(|user| {
                (
                    user.id,
                    TraceMentionUser {
                        user_id: user.id,
                        handle: user.handle.clone(),
                        display_name: user.display_name(),
                        profile_picture_display_url: user.profile_picture_display_url(pool),
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        let mut mentions_by_trace_id = HashMap::<Uuid, Vec<TraceMentionUser>>::new();
        for (trace_id, user_id, _) in rows {
            if let Some(summary) = summaries_by_user_id.get(&user_id) {
                mentions_by_trace_id
                    .entry(trace_id)
                    .or_default()
                    .push(summary.clone());
            }
        }
        Ok(mentions_by_trace_id)
    }

    pub fn mark_notified(
        trace_id: Uuid,
        mentioned_user_ids: &[Uuid],
        pool: &DbPool,
    ) -> Result<(), PpdcError> {
        if mentioned_user_ids.is_empty() {
            return Ok(());
        }
        let mut conn = pool.get()?;
        diesel::update(
            trace_mentions::table
                .filter(trace_mentions::trace_id.eq(trace_id))
                .filter(trace_mentions::mentioned_user_id.eq_any(mentioned_user_ids))
                .filter(trace_mentions::removed_at.is_null())
                .filter(trace_mentions::notified_at.is_null()),
        )
        .set((
            trace_mentions::notified_at.eq(diesel::dsl::now.nullable()),
            trace_mentions::updated_at.eq(diesel::dsl::now),
        ))
        .execute(&mut conn)?;
        Ok(())
    }
}

fn invalid_mention_target(user_id: Uuid) -> PpdcError {
    PpdcError::new(
        422,
        ErrorType::ApiError,
        "Mentioned user is not eligible".to_string(),
    )
    .with_details(serde_json::json!({
        "code": "invalid_trace_mention_target",
        "user_id": user_id,
    }))
}
