use diesel::pg::PgConnection;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::journal::{Journal, JournalStatus};
use crate::entities_v2::post::PostStatus;
use crate::entities_v2::post_grant::{
    PostGrant, PostGrantAccessLevel, PostGrantScope, PostGrantStatus,
};
use crate::entities_v2::relationship::Relationship;
use crate::entities_v2::trace::Trace;
use crate::entities_v2::user::{User, UserPrincipalType, UserRole};
use crate::schema::{journal_grants, post_grants, posts, relationships, traces, users};

use super::enums::{JournalGrantAccessLevel, JournalGrantScope, JournalGrantStatus};
use super::model::{JournalGrant, NewJournalGrantDto};

impl JournalGrant {
    fn to_post_grant_scope(grantee_scope: Option<JournalGrantScope>) -> Option<PostGrantScope> {
        match grantee_scope {
            Some(JournalGrantScope::AllPlatformUsers) => Some(PostGrantScope::AllPlatformUsers),
            None => None,
            Some(JournalGrantScope::AllAcceptedFollowers) => None,
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

    fn find_accepted_follower_ids_for_user_with_conn(
        user_id: Uuid,
        conn: &mut PgConnection,
    ) -> Result<Vec<Uuid>, PpdcError> {
        Ok(relationships::table
            .filter(relationships::target_user_id.eq(user_id))
            .filter(relationships::relationship_type.eq("FOLLOW"))
            .filter(relationships::status.eq("ACCEPTED"))
            .select(relationships::requester_user_id)
            .load::<Uuid>(conn)?)
    }

    fn is_follow_accepted_with_conn(
        follower_user_id: Uuid,
        target_user_id: Uuid,
        conn: &mut PgConnection,
    ) -> Result<bool, PpdcError> {
        let relationship_id = relationships::table
            .filter(relationships::requester_user_id.eq(follower_user_id))
            .filter(relationships::target_user_id.eq(target_user_id))
            .filter(relationships::relationship_type.eq("FOLLOW"))
            .filter(relationships::status.eq("ACCEPTED"))
            .select(relationships::id)
            .first::<Uuid>(conn)
            .optional()?;
        Ok(relationship_id.is_some())
    }

    fn find_auto_propagatable_post_ids_for_journal_with_conn(
        journal_id: Uuid,
        conn: &mut PgConnection,
    ) -> Result<Vec<Uuid>, PpdcError> {
        let query = posts::table
            .inner_join(traces::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .filter(traces::journal_id.eq(journal_id))
            .filter(traces::trace_type.eq("USER_TRACE"))
            .filter(traces::status.eq("FINALIZED"))
            .filter(posts::status.eq(PostStatus::Published.to_db()))
            .filter(traces::sharing_sensitivity.eq("NORMAL"))
            .select(posts::id)
            .distinct()
            .into_boxed();

        Ok(query.load::<Uuid>(conn)?)
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

        if grant.grantee_user_id.is_none() && grantee_scope.is_none() {
            return Ok(());
        }

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

    fn sync_all_active_journal_grants_to_post_with_conn(
        journal_id: Uuid,
        owner_user_id: Uuid,
        post_id: Uuid,
        trace_is_sensitive: bool,
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
            match (grant.grantee_user_id, grant.grantee_scope) {
                (Some(grantee_user_id), _) => {
                    if trace_is_sensitive {
                        continue;
                    }
                    if !Self::is_follow_accepted_with_conn(grantee_user_id, owner_user_id, conn)? {
                        continue;
                    }
                    PostGrant::upsert_direct_grants_for_posts_with_conn(
                        &[post_id],
                        owner_user_id,
                        grantee_user_id,
                        conn,
                    )?;
                }
                (None, Some(JournalGrantScope::AllAcceptedFollowers)) => {
                    if trace_is_sensitive {
                        continue;
                    }
                    let visible_follower_ids =
                        Self::find_accepted_follower_ids_for_user_with_conn(owner_user_id, conn)?;
                    for follower_user_id in visible_follower_ids {
                        PostGrant::upsert_direct_grants_for_posts_with_conn(
                            &[post_id],
                            owner_user_id,
                            follower_user_id,
                            conn,
                        )?;
                    }
                }
                (None, Some(JournalGrantScope::AllPlatformUsers)) => {
                    Self::create_or_update_synced_post_grant_with_conn(
                        post_id,
                        owner_user_id,
                        &grant,
                        conn,
                    )?;
                }
                (None, None) => {}
            }
        }

        Ok(())
    }

    fn sync_default_post_grants_for_journal_grant_with_conn(
        journal: &Journal,
        grant: &JournalGrant,
        conn: &mut PgConnection,
    ) -> Result<(), PpdcError> {
        let post_ids =
            Self::find_auto_propagatable_post_ids_for_journal_with_conn(journal.id, conn)?;

        match (grant.grantee_user_id, grant.grantee_scope) {
            (Some(grantee_user_id), _) => {
                if Self::is_follow_accepted_with_conn(grantee_user_id, journal.user_id, conn)? {
                    PostGrant::upsert_direct_grants_for_posts_with_conn(
                        &post_ids,
                        journal.user_id,
                        grantee_user_id,
                        conn,
                    )?;
                }
            }
            (None, Some(JournalGrantScope::AllAcceptedFollowers)) => {
                let follower_ids =
                    Self::find_accepted_follower_ids_for_user_with_conn(journal.user_id, conn)?;
                for follower_user_id in follower_ids {
                    PostGrant::upsert_direct_grants_for_posts_with_conn(
                        &post_ids,
                        journal.user_id,
                        follower_user_id,
                        conn,
                    )?;
                }
            }
            (None, Some(JournalGrantScope::AllPlatformUsers)) => {
                for post_id in post_ids {
                    Self::create_or_update_synced_post_grant_with_conn(
                        post_id,
                        journal.user_id,
                        grant,
                        conn,
                    )?;
                }
            }
            (None, None) => {}
        }

        Ok(())
    }

    fn revoke_default_post_grants_for_journal_grant_with_conn(
        journal_id: Uuid,
        grant: &JournalGrant,
        conn: &mut PgConnection,
    ) -> Result<(), PpdcError> {
        let post_ids =
            Self::find_auto_propagatable_post_ids_for_journal_with_conn(journal_id, conn)?;

        match (grant.grantee_user_id, grant.grantee_scope) {
            (Some(grantee_user_id), _) => {
                PostGrant::revoke_direct_grants_for_posts_with_conn(
                    &post_ids,
                    grant.owner_user_id,
                    grantee_user_id,
                    conn,
                )?;
            }
            (None, Some(JournalGrantScope::AllAcceptedFollowers)) => {
                let follower_ids =
                    Self::find_accepted_follower_ids_for_user_with_conn(grant.owner_user_id, conn)?;
                for follower_user_id in follower_ids {
                    PostGrant::revoke_direct_grants_for_posts_with_conn(
                        &post_ids,
                        grant.owner_user_id,
                        follower_user_id,
                        conn,
                    )?;
                }
            }
            (None, Some(JournalGrantScope::AllPlatformUsers)) => {
                for post_id in post_ids {
                    Self::revoke_synced_post_grant_with_conn(post_id, grant, conn)?;
                }
            }
            (None, None) => {}
        }

        Ok(())
    }

    pub(crate) fn sync_effective_post_grants_for_post(
        post: &crate::entities_v2::post::Post,
        pool: &DbPool,
    ) -> Result<(), PpdcError> {
        if post.status != PostStatus::Published {
            return Ok(());
        }

        let Some(trace_id) = post.source_trace_id else {
            return Ok(());
        };

        let trace = Trace::find_full_trace(trace_id, pool)?;
        let Some(journal_id) = trace.journal_id else {
            return Ok(());
        };

        let mut conn = pool.get()?;
        conn.transaction::<(), PpdcError, _>(|conn| {
            Self::sync_all_active_journal_grants_to_post_with_conn(
                journal_id,
                post.user_id,
                post.id,
                trace.sharing_sensitivity
                    != crate::entities_v2::trace::TraceSharingSensitivity::Normal,
                conn,
            )
        })?;

        Ok(())
    }

    pub(crate) fn propagate_follower_default_grants_for_pair_with_conn(
        owner_user_id: Uuid,
        follower_user_id: Uuid,
        conn: &mut PgConnection,
    ) -> Result<(), PpdcError> {
        let direct_journal_ids = journal_grants::table
            .filter(journal_grants::owner_user_id.eq(owner_user_id))
            .filter(journal_grants::grantee_user_id.eq(Some(follower_user_id)))
            .filter(journal_grants::status.eq(JournalGrantStatus::Active.to_db()))
            .select(journal_grants::journal_id)
            .distinct()
            .load::<Uuid>(conn)?;

        let follower_scope_journal_ids = journal_grants::table
            .filter(journal_grants::owner_user_id.eq(owner_user_id))
            .filter(
                journal_grants::grantee_scope
                    .eq(Some(JournalGrantScope::AllAcceptedFollowers.to_db())),
            )
            .filter(journal_grants::status.eq(JournalGrantStatus::Active.to_db()))
            .select(journal_grants::journal_id)
            .distinct()
            .load::<Uuid>(conn)?;

        let mut direct_post_ids = Vec::new();
        for journal_id in direct_journal_ids {
            direct_post_ids.extend(Self::find_auto_propagatable_post_ids_for_journal_with_conn(
                journal_id, conn,
            )?);
        }
        direct_post_ids.sort_unstable();
        direct_post_ids.dedup();
        PostGrant::upsert_direct_grants_for_posts_with_conn(
            &direct_post_ids,
            owner_user_id,
            follower_user_id,
            conn,
        )?;

        let mut follower_scope_post_ids = Vec::new();
        for journal_id in follower_scope_journal_ids {
            follower_scope_post_ids.extend(
                Self::find_auto_propagatable_post_ids_for_journal_with_conn(journal_id, conn)?,
            );
        }
        follower_scope_post_ids.sort_unstable();
        follower_scope_post_ids.dedup();

        PostGrant::upsert_direct_grants_for_posts_with_conn(
            &follower_scope_post_ids,
            owner_user_id,
            follower_user_id,
            conn,
        )?;

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
        let visible_post_ids = PostGrant::find_shared_post_ids_for_user(user_id, pool)?;
        if visible_post_ids.is_empty() {
            return Ok(false);
        }

        let mut conn = pool.get()?;
        let matching_post_id = posts::table
            .inner_join(traces::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .filter(traces::journal_id.eq(journal.id))
            .filter(posts::id.eq_any(visible_post_ids))
            .filter(posts::status.eq(PostStatus::Published.to_db()))
            .select(posts::id)
            .first::<Uuid>(&mut conn)
            .optional()?;

        Ok(matching_post_id.is_some())
    }

    pub fn find_shared_journal_ids_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Uuid>, PpdcError> {
        let visible_post_ids = PostGrant::find_shared_post_ids_for_user(user_id, pool)?;
        if visible_post_ids.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = pool.get()?;
        let ids = posts::table
            .inner_join(traces::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .filter(posts::id.eq_any(visible_post_ids))
            .filter(posts::status.eq(PostStatus::Published.to_db()))
            .filter(traces::journal_id.is_not_null())
            .select(traces::journal_id.assume_not_null())
            .distinct()
            .load::<Uuid>(&mut conn)?;

        Ok(ids)
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
