use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::sql_types::Uuid as SqlUuid;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::journal::{Journal, JournalStatus};
use crate::entities_v2::post::{PostAudienceRole, PostInteractionType, PostStatus, PostType};
use crate::entities_v2::post_grant::{PostGrantAccessLevel, PostGrantScope, PostGrantStatus};
use crate::entities_v2::relationship::{Relationship, RelationshipStatus};
use crate::entities_v2::shared::MaturingState;
use crate::entities_v2::trace::model::TraceRow;
use crate::entities_v2::trace::Trace;
use crate::entities_v2::user::{User, UserPrincipalType, UserRole};
use crate::schema::{journal_grants, post_grants, posts, users};

use super::enums::{JournalGrantAccessLevel, JournalGrantScope, JournalGrantStatus};
use super::model::{JournalGrant, NewJournalGrantDto};

impl JournalGrant {
    fn to_post_grant_scope(grantee_scope: Option<JournalGrantScope>) -> Option<PostGrantScope> {
        match grantee_scope {
            Some(JournalGrantScope::AllAcceptedFollowers) => {
                Some(PostGrantScope::AllAcceptedFollowers)
            }
            Some(JournalGrantScope::AllPlatformUsers) => Some(PostGrantScope::AllPlatformUsers),
            None => None,
        }
    }

    fn find_existing_with_conn(
        journal_id: Uuid,
        grantee_user_id: Option<Uuid>,
        grantee_scope: Option<JournalGrantScope>,
        conn: &mut PgConnection,
    ) -> Result<Option<JournalGrant>, PpdcError> {
        let mut query = journal_grants::table
            .filter(journal_grants::journal_id.eq(journal_id))
            .into_boxed();

        if let Some(grantee_user_id) = grantee_user_id {
            query = query.filter(journal_grants::grantee_user_id.eq(Some(grantee_user_id)));
        } else if let Some(scope) = grantee_scope {
            query = query.filter(journal_grants::grantee_scope.eq(Some(scope.to_db())));
        }

        let row = query
            .select((
                journal_grants::id,
                journal_grants::journal_id,
                journal_grants::owner_user_id,
                journal_grants::grantee_user_id,
                journal_grants::grantee_scope,
                journal_grants::access_level,
                journal_grants::status,
                journal_grants::created_at,
                journal_grants::updated_at,
            ))
            .first::<(
                Uuid,
                Uuid,
                Uuid,
                Option<Uuid>,
                Option<String>,
                String,
                String,
                chrono::NaiveDateTime,
                chrono::NaiveDateTime,
            )>(conn)
            .optional()?;

        row.map(|row| {
            Ok(JournalGrant {
                id: row.0,
                journal_id: row.1,
                owner_user_id: row.2,
                grantee_user_id: row.3,
                grantee_scope: row.4.as_deref().and_then(JournalGrantScope::from_db),
                access_level: JournalGrantAccessLevel::from_db(&row.5),
                status: JournalGrantStatus::from_db(&row.6),
                created_at: row.7,
                updated_at: row.8,
            })
        })
        .transpose()
    }

    fn owner_can_use_scope(
        owner_user_id: Uuid,
        grantee_scope: Option<JournalGrantScope>,
        pool: &DbPool,
    ) -> Result<(), PpdcError> {
        if grantee_scope != Some(JournalGrantScope::AllPlatformUsers) {
            return Ok(());
        }

        let owner = User::find(&owner_user_id, pool)?;
        if !owner.has_role(UserRole::Admin, pool)? {
            return Err(PpdcError::new(
                403,
                ErrorType::ApiError,
                "ALL_PLATFORM_USERS journal grants are restricted to admins".to_string(),
            ));
        }

        Ok(())
    }

    fn list_platform_human_user_ids(pool: &DbPool) -> Result<Vec<Uuid>, PpdcError> {
        let mut conn = pool.get()?;

        let ids = users::table
            .filter(users::is_platform_user.eq(true))
            .filter(users::principal_type.eq(UserPrincipalType::Human))
            .select(users::id)
            .load::<Uuid>(&mut conn)?;
        Ok(ids)
    }

    fn validate_target_fields(
        payload: &NewJournalGrantDto,
    ) -> Result<
        (
            Option<Uuid>,
            Option<JournalGrantScope>,
            JournalGrantAccessLevel,
        ),
        PpdcError,
    > {
        match (payload.grantee_user_id, payload.grantee_scope) {
            (Some(grantee_user_id), None) => Ok((
                Some(grantee_user_id),
                None,
                payload
                    .access_level
                    .unwrap_or(JournalGrantAccessLevel::Read),
            )),
            (None, Some(scope)) => Ok((
                None,
                Some(scope),
                payload
                    .access_level
                    .unwrap_or(JournalGrantAccessLevel::Read),
            )),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Provide either grantee_user_id or grantee_scope".to_string(),
            )),
        }
    }

    fn find_finalized_user_traces_for_journal_with_conn(
        journal_id: Uuid,
        conn: &mut PgConnection,
    ) -> Result<Vec<Trace>, PpdcError> {
        let rows = diesel::sql_query(
            "SELECT id, title, subtitle, interaction_date, content, is_encrypted, encryption_metadata::text AS encryption_metadata, image_asset_id, timeout_start_at, timeout_at, journal_id, user_id, trace_type, status, start_writing_at, finalized_at, created_at, updated_at
             FROM traces
             WHERE journal_id = $1
               AND trace_type = 'USER_TRACE'
               AND status = 'FINALIZED'
             ORDER BY interaction_date DESC NULLS LAST, created_at DESC",
        )
        .bind::<SqlUuid, _>(journal_id)
        .load::<TraceRow>(conn)?;

        Ok(rows.into_iter().map(Trace::from).collect())
    }

    fn find_default_post_for_trace_with_conn(
        trace_id: Uuid,
        conn: &mut PgConnection,
    ) -> Result<Option<(Uuid, PostStatus)>, PpdcError> {
        let row = posts::table
            .filter(posts::source_trace_id.eq(Some(trace_id)))
            .filter(posts::audience_role.eq(PostAudienceRole::Default.to_db()))
            .select((posts::id, posts::status))
            .order(posts::created_at.desc())
            .first::<(Uuid, String)>(conn)
            .optional()?;

        Ok(row.map(|(post_id, status_raw)| (post_id, PostStatus::from_db(&status_raw))))
    }

    fn find_matching_post_grant_id_with_conn(
        post_id: Uuid,
        grantee_user_id: Option<Uuid>,
        grantee_scope: Option<PostGrantScope>,
        conn: &mut PgConnection,
    ) -> Result<Option<Uuid>, PpdcError> {
        let mut query = post_grants::table
            .filter(post_grants::post_id.eq(post_id))
            .into_boxed();

        if let Some(grantee_user_id) = grantee_user_id {
            query = query.filter(post_grants::grantee_user_id.eq(Some(grantee_user_id)));
        } else if let Some(grantee_scope) = grantee_scope {
            query = query.filter(post_grants::grantee_scope.eq(Some(grantee_scope.to_db())));
        }

        Ok(query
            .select(post_grants::id)
            .first::<Uuid>(conn)
            .optional()?)
    }

    fn create_or_update_synced_post_grant_with_conn(
        post_id: Uuid,
        owner_user_id: Uuid,
        grant: &JournalGrant,
        conn: &mut PgConnection,
    ) -> Result<(), PpdcError> {
        let grantee_scope = Self::to_post_grant_scope(grant.grantee_scope);

        if let Some(existing_id) = Self::find_matching_post_grant_id_with_conn(
            post_id,
            grant.grantee_user_id,
            grantee_scope,
            conn,
        )? {
            diesel::update(post_grants::table.filter(post_grants::id.eq(existing_id)))
                .set((
                    post_grants::access_level.eq(PostGrantAccessLevel::Read.to_db()),
                    post_grants::status.eq(PostGrantStatus::Active.to_db()),
                    post_grants::updated_at.eq(diesel::dsl::now),
                ))
                .execute(conn)?;
            return Ok(());
        }

        diesel::insert_into(post_grants::table)
            .values((
                post_grants::id.eq(Uuid::new_v4()),
                post_grants::post_id.eq(post_id),
                post_grants::owner_user_id.eq(owner_user_id),
                post_grants::grantee_user_id.eq(grant.grantee_user_id),
                post_grants::grantee_scope.eq(grantee_scope.map(|scope| scope.to_db())),
                post_grants::access_level.eq(PostGrantAccessLevel::Read.to_db()),
                post_grants::status.eq(PostGrantStatus::Active.to_db()),
            ))
            .execute(conn)?;

        Ok(())
    }

    fn revoke_synced_post_grant_with_conn(
        post_id: Uuid,
        grant: &JournalGrant,
        conn: &mut PgConnection,
    ) -> Result<(), PpdcError> {
        let grantee_scope = Self::to_post_grant_scope(grant.grantee_scope);
        let mut query = post_grants::table
            .filter(post_grants::post_id.eq(post_id))
            .into_boxed();

        if let Some(grantee_user_id) = grant.grantee_user_id {
            query = query.filter(post_grants::grantee_user_id.eq(Some(grantee_user_id)));
        } else if let Some(grantee_scope) = grantee_scope {
            query = query.filter(post_grants::grantee_scope.eq(Some(grantee_scope.to_db())));
        }

        let grant_ids = query.select(post_grants::id).load::<Uuid>(conn)?;
        if !grant_ids.is_empty() {
            diesel::update(post_grants::table.filter(post_grants::id.eq_any(grant_ids)))
                .set((
                    post_grants::status.eq(PostGrantStatus::Revoked.to_db()),
                    post_grants::updated_at.eq(diesel::dsl::now),
                ))
                .execute(conn)?;
        }
        Ok(())
    }

    fn create_default_published_post_for_trace_with_conn(
        trace: &Trace,
        conn: &mut PgConnection,
    ) -> Result<Uuid, PpdcError> {
        let publishing_date = trace.finalized_at.or(Some(trace.created_at));

        let post_id = diesel::insert_into(posts::table)
            .values((
                posts::source_trace_id.eq(Some(trace.id)),
                posts::title.eq(trace.title.clone()),
                posts::subtitle.eq(trace.subtitle.clone()),
                posts::content.eq(trace.content.clone()),
                posts::image_url.eq(None::<String>),
                posts::image_asset_id.eq(trace.image_asset_id),
                posts::interaction_type.eq(PostInteractionType::Output.to_db()),
                posts::post_type.eq(PostType::Idea.to_db()),
                posts::user_id.eq(trace.user_id),
                posts::publishing_date.eq(publishing_date),
                posts::status.eq(PostStatus::Published.to_db()),
                posts::audience_role.eq(PostAudienceRole::Default.to_db()),
                posts::publishing_state.eq("pbsh"),
                posts::maturing_state.eq(MaturingState::Finished.to_code()),
            ))
            .returning(posts::id)
            .get_result::<Uuid>(conn)?;

        Ok(post_id)
    }

    fn sync_all_active_journal_grants_to_post_with_conn(
        journal_id: Uuid,
        owner_user_id: Uuid,
        post_id: Uuid,
        conn: &mut PgConnection,
    ) -> Result<(), PpdcError> {
        let grant_rows = journal_grants::table
            .filter(journal_grants::journal_id.eq(journal_id))
            .filter(journal_grants::status.eq(JournalGrantStatus::Active.to_db()))
            .select((
                journal_grants::id,
                journal_grants::journal_id,
                journal_grants::owner_user_id,
                journal_grants::grantee_user_id,
                journal_grants::grantee_scope,
                journal_grants::access_level,
                journal_grants::status,
                journal_grants::created_at,
                journal_grants::updated_at,
            ))
            .load::<(
                Uuid,
                Uuid,
                Uuid,
                Option<Uuid>,
                Option<String>,
                String,
                String,
                chrono::NaiveDateTime,
                chrono::NaiveDateTime,
            )>(conn)?;

        for row in grant_rows {
            let grant = JournalGrant {
                id: row.0,
                journal_id: row.1,
                owner_user_id: row.2,
                grantee_user_id: row.3,
                grantee_scope: row.4.as_deref().and_then(JournalGrantScope::from_db),
                access_level: JournalGrantAccessLevel::from_db(&row.5),
                status: JournalGrantStatus::from_db(&row.6),
                created_at: row.7,
                updated_at: row.8,
            };
            Self::create_or_update_synced_post_grant_with_conn(
                post_id,
                owner_user_id,
                &grant,
                conn,
            )?;
        }

        Ok(())
    }

    fn sync_default_post_grants_for_journal_grant_with_conn(
        journal: &Journal,
        grant: &JournalGrant,
        conn: &mut PgConnection,
    ) -> Result<(), PpdcError> {
        let traces = Self::find_finalized_user_traces_for_journal_with_conn(journal.id, conn)?;

        for trace in traces {
            match Self::find_default_post_for_trace_with_conn(trace.id, conn)? {
                Some((_, PostStatus::Archived)) => {}
                Some((post_id, _)) => {
                    Self::create_or_update_synced_post_grant_with_conn(
                        post_id,
                        journal.user_id,
                        grant,
                        conn,
                    )?;
                }
                None => {
                    let post_id =
                        Self::create_default_published_post_for_trace_with_conn(&trace, conn)?;
                    Self::sync_all_active_journal_grants_to_post_with_conn(
                        journal.id,
                        journal.user_id,
                        post_id,
                        conn,
                    )?;
                }
            }
        }

        Ok(())
    }

    fn revoke_default_post_grants_for_journal_grant_with_conn(
        journal_id: Uuid,
        grant: &JournalGrant,
        conn: &mut PgConnection,
    ) -> Result<(), PpdcError> {
        let traces = Self::find_finalized_user_traces_for_journal_with_conn(journal_id, conn)?;

        for trace in traces {
            if let Some((post_id, status)) =
                Self::find_default_post_for_trace_with_conn(trace.id, conn)?
            {
                if status == PostStatus::Archived {
                    continue;
                }
                Self::revoke_synced_post_grant_with_conn(post_id, grant, conn)?;
            }
        }

        Ok(())
    }

    pub fn create_or_update(
        journal: &Journal,
        owner_user_id: Uuid,
        payload: NewJournalGrantDto,
        pool: &DbPool,
    ) -> Result<(JournalGrant, bool), PpdcError> {
        if journal.user_id != owner_user_id {
            return Err(PpdcError::unauthorized());
        }
        if journal.is_encrypted {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Encrypted journals cannot be shared".to_string(),
            ));
        }

        let (grantee_user_id, grantee_scope, access_level) =
            JournalGrant::validate_target_fields(&payload)?;
        JournalGrant::owner_can_use_scope(owner_user_id, grantee_scope, pool)?;

        if let Some(grantee_user_id) = grantee_user_id {
            if grantee_user_id == owner_user_id {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "Cannot grant a journal to yourself".to_string(),
                ));
            }
            let accepted = Relationship::is_follow_accepted(grantee_user_id, owner_user_id, pool)?;
            if !accepted {
                return Err(PpdcError::new(
                    403,
                    ErrorType::ApiError,
                    "Direct journal grants are restricted to accepted followers".to_string(),
                ));
            }
        }

        let mut conn = pool.get()?;
        let (grant_id, should_notify) = conn.transaction::<(Uuid, bool), PpdcError, _>(|conn| {
            if let Some(existing) =
                Self::find_existing_with_conn(journal.id, grantee_user_id, grantee_scope, conn)?
            {
                let should_notify = existing.grantee_user_id.is_some()
                    && existing.status != JournalGrantStatus::Active;
                diesel::update(journal_grants::table.filter(journal_grants::id.eq(existing.id)))
                    .set((
                        journal_grants::access_level.eq(access_level.to_db()),
                        journal_grants::status.eq(JournalGrantStatus::Active.to_db()),
                        journal_grants::updated_at.eq(diesel::dsl::now),
                    ))
                    .execute(conn)?;

                let updated_grant = Self::find_existing_with_conn(
                    journal.id,
                    grantee_user_id,
                    grantee_scope,
                    conn,
                )?
                .ok_or_else(|| {
                    PpdcError::new(
                        404,
                        ErrorType::ApiError,
                        "Journal grant not found".to_string(),
                    )
                })?;

                Self::sync_default_post_grants_for_journal_grant_with_conn(
                    journal,
                    &updated_grant,
                    conn,
                )?;

                return Ok((existing.id, should_notify));
            }

            let id = Uuid::new_v4();
            diesel::insert_into(journal_grants::table)
                .values((
                    journal_grants::id.eq(id),
                    journal_grants::journal_id.eq(journal.id),
                    journal_grants::owner_user_id.eq(owner_user_id),
                    journal_grants::grantee_user_id.eq(grantee_user_id),
                    journal_grants::grantee_scope.eq(grantee_scope.map(|scope| scope.to_db())),
                    journal_grants::access_level.eq(access_level.to_db()),
                    journal_grants::status.eq(JournalGrantStatus::Active.to_db()),
                ))
                .execute(conn)?;

            let created_grant =
                Self::find_existing_with_conn(journal.id, grantee_user_id, grantee_scope, conn)?
                    .ok_or_else(|| {
                        PpdcError::new(
                            404,
                            ErrorType::ApiError,
                            "Journal grant not found".to_string(),
                        )
                    })?;

            Self::sync_default_post_grants_for_journal_grant_with_conn(
                journal,
                &created_grant,
                conn,
            )?;

            Ok((id, grantee_user_id.is_some()))
        })?;

        Ok((JournalGrant::find(grant_id, pool)?, should_notify))
    }

    pub fn revoke(
        journal_id: Uuid,
        grant_id: Uuid,
        owner_user_id: Uuid,
        pool: &DbPool,
    ) -> Result<JournalGrant, PpdcError> {
        let journal = Journal::find_full(journal_id, pool)?;
        if journal.user_id != owner_user_id {
            return Err(PpdcError::unauthorized());
        }
        let grant = JournalGrant::find(grant_id, pool)?;
        if grant.journal_id != journal_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Grant does not belong to the specified journal".to_string(),
            ));
        }

        let mut conn = pool.get()?;
        conn.transaction::<(), PpdcError, _>(|conn| {
            diesel::update(journal_grants::table.filter(journal_grants::id.eq(grant_id)))
                .set((
                    journal_grants::status.eq(JournalGrantStatus::Revoked.to_db()),
                    journal_grants::updated_at.eq(diesel::dsl::now),
                ))
                .execute(conn)?;

            Self::revoke_default_post_grants_for_journal_grant_with_conn(journal_id, &grant, conn)?;
            Ok(())
        })?;
        JournalGrant::find(grant_id, pool)
    }

    pub fn user_can_read_journal(
        journal: &Journal,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
        if journal.user_id == user_id {
            return Ok(true);
        }
        if journal.status == JournalStatus::Archived {
            return Ok(false);
        }
        if journal.is_encrypted {
            return Ok(false);
        }

        let grants = JournalGrant::find_for_journal(journal.id, pool)?;
        for grant in grants
            .into_iter()
            .filter(|grant| grant.status == JournalGrantStatus::Active)
        {
            if grant.grantee_user_id == Some(user_id) {
                return Ok(true);
            }

            if grant.grantee_scope == Some(JournalGrantScope::AllAcceptedFollowers)
                && matches!(
                    Relationship::find_between(
                        user_id,
                        journal.user_id,
                        crate::entities_v2::relationship::RelationshipType::Follow,
                        pool,
                    )?,
                    Some(Relationship {
                        status: RelationshipStatus::Accepted,
                        ..
                    })
                )
            {
                return Ok(true);
            }

            if grant.grantee_scope == Some(JournalGrantScope::AllPlatformUsers) {
                let user = User::find(&user_id, pool)?;
                if user.is_platform_user && user.principal_type == UserPrincipalType::Human {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    pub fn find_shared_journal_ids_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Uuid>, PpdcError> {
        use std::collections::HashSet;

        let mut ids = HashSet::new();
        let mut conn = pool.get()?;
        let direct_ids = journal_grants::table
            .filter(journal_grants::grantee_user_id.eq(Some(user_id)))
            .filter(journal_grants::status.eq(JournalGrantStatus::Active.to_db()))
            .select(journal_grants::journal_id)
            .load::<Uuid>(&mut conn)?;
        ids.extend(direct_ids);

        let follower_scope_grants = journal_grants::table
            .filter(
                journal_grants::grantee_scope
                    .eq(Some(JournalGrantScope::AllAcceptedFollowers.to_db())),
            )
            .filter(journal_grants::status.eq(JournalGrantStatus::Active.to_db()))
            .select((journal_grants::journal_id, journal_grants::owner_user_id))
            .load::<(Uuid, Uuid)>(&mut conn)?;

        for (journal_id, owner_user_id) in follower_scope_grants {
            if Relationship::is_follow_accepted(user_id, owner_user_id, pool)? {
                ids.insert(journal_id);
            }
        }

        let platform_scope_journal_ids = journal_grants::table
            .filter(
                journal_grants::grantee_scope.eq(Some(JournalGrantScope::AllPlatformUsers.to_db())),
            )
            .filter(journal_grants::status.eq(JournalGrantStatus::Active.to_db()))
            .select(journal_grants::journal_id)
            .load::<Uuid>(&mut conn)?;

        let user = User::find(&user_id, pool)?;
        if user.is_platform_user && user.principal_type == UserPrincipalType::Human {
            ids.extend(platform_scope_journal_ids);
        }

        Ok(ids.into_iter().collect())
    }

    pub fn find_active_recipient_user_ids_for_journal(
        journal: &Journal,
        pool: &DbPool,
    ) -> Result<Vec<Uuid>, PpdcError> {
        use std::collections::HashSet;

        let mut ids = HashSet::new();
        let grants = JournalGrant::find_for_journal(journal.id, pool)?;

        for grant in grants
            .into_iter()
            .filter(|grant| grant.status == JournalGrantStatus::Active)
        {
            if let Some(grantee_user_id) = grant.grantee_user_id {
                ids.insert(grantee_user_id);
            }

            if grant.grantee_scope == Some(JournalGrantScope::AllAcceptedFollowers) {
                for relationship in Relationship::find_followers_for_user(journal.user_id, pool)? {
                    ids.insert(relationship.requester_user_id);
                }
            }

            if grant.grantee_scope == Some(JournalGrantScope::AllPlatformUsers) {
                ids.extend(Self::list_platform_human_user_ids(pool)?);
            }
        }

        ids.remove(&journal.user_id);
        Ok(ids.into_iter().collect())
    }

    pub fn has_active_grants_for_journal(
        journal_id: Uuid,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
        let mut conn = pool.get()?;
        let count = journal_grants::table
            .filter(journal_grants::journal_id.eq(journal_id))
            .filter(journal_grants::status.eq(JournalGrantStatus::Active.to_db()))
            .count()
            .get_result::<i64>(&mut conn)?;
        Ok(count > 0)
    }
}
