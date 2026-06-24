use diesel::pg::PgConnection;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    journal::{Journal, JournalSharingMode, JournalStatus},
    post::{Post, PostStatus},
    post_grant::PostGrant,
    relationship::Relationship,
    trace::{Trace, TraceSharingSensitivity, TraceStatus, TraceType},
    user::{User, UserPublicResponse},
};
use crate::schema::{journal_sharing_policies, journals, posts, traces};

use super::enums::{JournalHistoryDecision, JournalHistoryReviewState, JournalSharingPolicyStatus};
use super::model::{
    JournalSharingPolicy, JournalSharingPolicyHistoryDecisionDto,
    JournalSharingPolicyPendingReview, JournalSharingPolicyReviewJournal,
    JournalSharingPolicyReviewReason, NewJournalSharingPolicyDto, UpdateJournalSharingPolicyDto,
};

type JournalSharingPolicyTuple = (
    Uuid,
    Uuid,
    Uuid,
    Uuid,
    String,
    bool,
    String,
    Option<String>,
    Option<chrono::NaiveDateTime>,
    chrono::NaiveDateTime,
    chrono::NaiveDateTime,
);

fn tuple_to_policy(row: JournalSharingPolicyTuple) -> JournalSharingPolicy {
    JournalSharingPolicy {
        id: row.0,
        journal_id: row.1,
        owner_user_id: row.2,
        grantee_user_id: row.3,
        status: JournalSharingPolicyStatus::from_db(&row.4),
        default_future_access_enabled: row.5,
        history_review_state: JournalHistoryReviewState::from_db(&row.6),
        history_decision: row.7.as_deref().and_then(JournalHistoryDecision::from_db),
        history_reviewed_at: row.8,
        created_at: row.9,
        updated_at: row.10,
    }
}

fn select_policy_columns() -> (
    journal_sharing_policies::id,
    journal_sharing_policies::journal_id,
    journal_sharing_policies::owner_user_id,
    journal_sharing_policies::grantee_user_id,
    journal_sharing_policies::status,
    journal_sharing_policies::default_future_access_enabled,
    journal_sharing_policies::history_review_state,
    journal_sharing_policies::history_decision,
    journal_sharing_policies::history_reviewed_at,
    journal_sharing_policies::created_at,
    journal_sharing_policies::updated_at,
) {
    (
        journal_sharing_policies::id,
        journal_sharing_policies::journal_id,
        journal_sharing_policies::owner_user_id,
        journal_sharing_policies::grantee_user_id,
        journal_sharing_policies::status,
        journal_sharing_policies::default_future_access_enabled,
        journal_sharing_policies::history_review_state,
        journal_sharing_policies::history_decision,
        journal_sharing_policies::history_reviewed_at,
        journal_sharing_policies::created_at,
        journal_sharing_policies::updated_at,
    )
}

fn is_follow_accepted_with_conn(
    follower_user_id: Uuid,
    target_user_id: Uuid,
    conn: &mut PgConnection,
) -> Result<bool, PpdcError> {
    let relationship_id = crate::schema::relationships::table
        .filter(crate::schema::relationships::requester_user_id.eq(follower_user_id))
        .filter(crate::schema::relationships::target_user_id.eq(target_user_id))
        .filter(crate::schema::relationships::relationship_type.eq("FOLLOW"))
        .filter(crate::schema::relationships::status.eq("ACCEPTED"))
        .select(crate::schema::relationships::id)
        .first::<Uuid>(conn)
        .optional()?;
    Ok(relationship_id.is_some())
}

fn find_accepted_follower_ids_for_user_with_conn(
    owner_user_id: Uuid,
    conn: &mut PgConnection,
) -> Result<Vec<Uuid>, PpdcError> {
    let rows = crate::schema::relationships::table
        .filter(crate::schema::relationships::target_user_id.eq(owner_user_id))
        .filter(crate::schema::relationships::relationship_type.eq("FOLLOW"))
        .filter(crate::schema::relationships::status.eq("ACCEPTED"))
        .select(crate::schema::relationships::requester_user_id)
        .distinct()
        .load::<Uuid>(conn)?;
    Ok(rows)
}

fn default_policy_values_for_sharing_mode(
    sharing_mode: JournalSharingMode,
) -> Option<(JournalSharingPolicyStatus, bool, JournalHistoryReviewState)> {
    match sharing_mode {
        JournalSharingMode::Shared => Some((
            JournalSharingPolicyStatus::Active,
            true,
            JournalHistoryReviewState::Unreviewed,
        )),
        JournalSharingMode::SemiShared => Some((
            JournalSharingPolicyStatus::Suggested,
            false,
            JournalHistoryReviewState::NotStarted,
        )),
        JournalSharingMode::Private => None,
    }
}

fn create_policy_if_absent_with_conn(
    journal_id: Uuid,
    owner_user_id: Uuid,
    grantee_user_id: Uuid,
    status: JournalSharingPolicyStatus,
    default_future_access_enabled: bool,
    history_review_state: JournalHistoryReviewState,
    conn: &mut PgConnection,
) -> Result<(), PpdcError> {
    let existing_id = journal_sharing_policies::table
        .filter(journal_sharing_policies::journal_id.eq(journal_id))
        .filter(journal_sharing_policies::grantee_user_id.eq(grantee_user_id))
        .select(journal_sharing_policies::id)
        .first::<Uuid>(conn)
        .optional()?;
    if existing_id.is_some() {
        return Ok(());
    }

    diesel::insert_into(journal_sharing_policies::table)
        .values((
            journal_sharing_policies::id.eq(Uuid::new_v4()),
            journal_sharing_policies::journal_id.eq(journal_id),
            journal_sharing_policies::owner_user_id.eq(owner_user_id),
            journal_sharing_policies::grantee_user_id.eq(grantee_user_id),
            journal_sharing_policies::status.eq(status.to_db()),
            journal_sharing_policies::default_future_access_enabled
                .eq(default_future_access_enabled),
            journal_sharing_policies::history_review_state.eq(history_review_state.to_db()),
        ))
        .execute(conn)?;
    Ok(())
}

impl JournalSharingPolicy {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<JournalSharingPolicy, PpdcError> {
        let mut conn = pool.get()?;
        let row = journal_sharing_policies::table
            .filter(journal_sharing_policies::id.eq(id))
            .select(select_policy_columns())
            .first::<JournalSharingPolicyTuple>(&mut conn)
            .optional()?;
        row.map(tuple_to_policy).ok_or_else(|| {
            PpdcError::new(
                404,
                ErrorType::ApiError,
                "Journal sharing policy not found".to_string(),
            )
        })
    }

    pub fn find_for_journal_paginated(
        journal_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<JournalSharingPolicy>, i64), PpdcError> {
        let mut conn = pool.get()?;
        let total = journal_sharing_policies::table
            .filter(journal_sharing_policies::journal_id.eq(journal_id))
            .count()
            .get_result::<i64>(&mut conn)?;
        let rows = journal_sharing_policies::table
            .filter(journal_sharing_policies::journal_id.eq(journal_id))
            .select(select_policy_columns())
            .order(journal_sharing_policies::updated_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<JournalSharingPolicyTuple>(&mut conn)?;
        Ok((rows.into_iter().map(tuple_to_policy).collect(), total))
    }

    pub fn find_pending_reviews_for_owner_paginated(
        owner_user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<JournalSharingPolicyPendingReview>, i64), PpdcError> {
        let mut conn = pool.get()?;
        let pending_filter = journal_sharing_policies::status
            .eq(JournalSharingPolicyStatus::Suggested.to_db())
            .or(journal_sharing_policies::status
                .eq(JournalSharingPolicyStatus::Active.to_db())
                .and(
                    journal_sharing_policies::history_review_state
                        .ne(JournalHistoryReviewState::Reviewed.to_db()),
                ));

        let total = journal_sharing_policies::table
            .filter(journal_sharing_policies::owner_user_id.eq(owner_user_id))
            .filter(pending_filter.clone())
            .count()
            .get_result::<i64>(&mut conn)?;

        let rows = journal_sharing_policies::table
            .filter(journal_sharing_policies::owner_user_id.eq(owner_user_id))
            .filter(pending_filter)
            .select(select_policy_columns())
            .order(journal_sharing_policies::updated_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<JournalSharingPolicyTuple>(&mut conn)?;

        let mut items = Vec::with_capacity(rows.len());
        for policy in rows.into_iter().map(tuple_to_policy) {
            let journal = Journal::find_full(policy.journal_id, pool)?;
            let grantee = User::find(&policy.grantee_user_id, pool)?;
            let mut review_reasons = Vec::new();
            if policy.status == JournalSharingPolicyStatus::Suggested {
                review_reasons.push(JournalSharingPolicyReviewReason::FutureDefault);
            }
            if policy.history_review_state != JournalHistoryReviewState::Reviewed {
                review_reasons.push(JournalSharingPolicyReviewReason::History);
            }

            items.push(JournalSharingPolicyPendingReview {
                policy,
                journal: JournalSharingPolicyReviewJournal {
                    id: journal.id,
                    title: journal.title,
                    subtitle: journal.subtitle,
                    sharing_mode: journal.sharing_mode,
                },
                grantee: UserPublicResponse::from(&grantee),
                review_reasons,
            });
        }

        Ok((items, total))
    }

    pub fn has_active_policies_for_journal(
        journal_id: Uuid,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
        let mut conn = pool.get()?;
        let count = journal_sharing_policies::table
            .filter(journal_sharing_policies::journal_id.eq(journal_id))
            .filter(journal_sharing_policies::status.eq(JournalSharingPolicyStatus::Active.to_db()))
            .count()
            .get_result::<i64>(&mut conn)?;
        Ok(count > 0)
    }

    pub fn user_can_read_journal(
        journal: &Journal,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
        if journal.user_id == user_id {
            return Ok(true);
        }
        if journal.status == JournalStatus::Archived || journal.is_encrypted {
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

    pub fn create_or_update(
        journal: &Journal,
        owner_user_id: Uuid,
        payload: NewJournalSharingPolicyDto,
        pool: &DbPool,
    ) -> Result<JournalSharingPolicy, PpdcError> {
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
        if payload.grantee_user_id == owner_user_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Cannot create a journal sharing policy for yourself".to_string(),
            ));
        }
        if !Relationship::is_follow_accepted(payload.grantee_user_id, owner_user_id, pool)? {
            return Err(PpdcError::new(
                403,
                ErrorType::ApiError,
                "Journal sharing policies are restricted to accepted followers".to_string(),
            ));
        }

        let status = payload.status.unwrap_or(JournalSharingPolicyStatus::Active);
        let default_future_access_enabled = payload
            .default_future_access_enabled
            .unwrap_or(status == JournalSharingPolicyStatus::Active);
        let history_review_state = if status == JournalSharingPolicyStatus::Active {
            JournalHistoryReviewState::Unreviewed
        } else {
            JournalHistoryReviewState::NotStarted
        };

        let mut conn = pool.get()?;
        let id = conn.transaction::<Uuid, PpdcError, _>(|conn| {
            let existing_id = journal_sharing_policies::table
                .filter(journal_sharing_policies::journal_id.eq(journal.id))
                .filter(journal_sharing_policies::grantee_user_id.eq(payload.grantee_user_id))
                .select(journal_sharing_policies::id)
                .first::<Uuid>(conn)
                .optional()?;

            if let Some(existing_id) = existing_id {
                diesel::update(
                    journal_sharing_policies::table
                        .filter(journal_sharing_policies::id.eq(existing_id)),
                )
                .set((
                    journal_sharing_policies::status.eq(status.to_db()),
                    journal_sharing_policies::default_future_access_enabled
                        .eq(default_future_access_enabled),
                    journal_sharing_policies::history_review_state.eq(history_review_state.to_db()),
                    journal_sharing_policies::updated_at.eq(diesel::dsl::now),
                ))
                .execute(conn)?;
                return Ok(existing_id);
            }

            let id = Uuid::new_v4();
            diesel::insert_into(journal_sharing_policies::table)
                .values((
                    journal_sharing_policies::id.eq(id),
                    journal_sharing_policies::journal_id.eq(journal.id),
                    journal_sharing_policies::owner_user_id.eq(owner_user_id),
                    journal_sharing_policies::grantee_user_id.eq(payload.grantee_user_id),
                    journal_sharing_policies::status.eq(status.to_db()),
                    journal_sharing_policies::default_future_access_enabled
                        .eq(default_future_access_enabled),
                    journal_sharing_policies::history_review_state.eq(history_review_state.to_db()),
                ))
                .execute(conn)?;
            Ok(id)
        })?;

        Self::find(id, pool)
    }

    pub fn update(
        journal_id: Uuid,
        policy_id: Uuid,
        owner_user_id: Uuid,
        payload: UpdateJournalSharingPolicyDto,
        pool: &DbPool,
    ) -> Result<JournalSharingPolicy, PpdcError> {
        let journal = Journal::find_full(journal_id, pool)?;
        if journal.user_id != owner_user_id {
            return Err(PpdcError::unauthorized());
        }
        let policy = Self::find(policy_id, pool)?;
        if policy.journal_id != journal_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Sharing policy does not belong to the specified journal".to_string(),
            ));
        }

        let status = payload.status.unwrap_or(policy.status);
        let default_future_access_enabled = payload
            .default_future_access_enabled
            .unwrap_or(policy.default_future_access_enabled);

        let mut conn = pool.get()?;
        diesel::update(
            journal_sharing_policies::table.filter(journal_sharing_policies::id.eq(policy_id)),
        )
        .set((
            journal_sharing_policies::status.eq(status.to_db()),
            journal_sharing_policies::default_future_access_enabled
                .eq(default_future_access_enabled),
            journal_sharing_policies::updated_at.eq(diesel::dsl::now),
        ))
        .execute(&mut conn)?;

        Self::find(policy_id, pool)
    }

    pub fn revoke(
        journal_id: Uuid,
        policy_id: Uuid,
        owner_user_id: Uuid,
        pool: &DbPool,
    ) -> Result<JournalSharingPolicy, PpdcError> {
        Self::update(
            journal_id,
            policy_id,
            owner_user_id,
            UpdateJournalSharingPolicyDto {
                status: Some(JournalSharingPolicyStatus::Revoked),
                default_future_access_enabled: Some(false),
            },
            pool,
        )
    }

    pub fn apply_history_decision(
        journal_id: Uuid,
        policy_id: Uuid,
        owner_user_id: Uuid,
        payload: JournalSharingPolicyHistoryDecisionDto,
        pool: &DbPool,
    ) -> Result<JournalSharingPolicy, PpdcError> {
        let journal = Journal::find_full(journal_id, pool)?;
        if journal.user_id != owner_user_id {
            return Err(PpdcError::unauthorized());
        }
        if journal.is_encrypted {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Encrypted journals cannot propagate history grants".to_string(),
            ));
        }

        let policy = Self::find(policy_id, pool)?;
        if policy.journal_id != journal_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Sharing policy does not belong to the specified journal".to_string(),
            ));
        }
        if !matches!(
            policy.status,
            JournalSharingPolicyStatus::Suggested | JournalSharingPolicyStatus::Active
        ) {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "History decisions are only allowed for suggested or active policies".to_string(),
            ));
        }
        if !Relationship::is_follow_accepted(policy.grantee_user_id, owner_user_id, pool)? {
            return Err(PpdcError::new(
                403,
                ErrorType::ApiError,
                "History propagation requires an accepted follower".to_string(),
            ));
        }

        let mut conn = pool.get()?;
        conn.transaction::<(), PpdcError, _>(|conn| {
            let post_ids = match payload.decision {
                JournalHistoryDecision::None => {
                    if payload.post_ids.is_some() {
                        return Err(PpdcError::new(
                            400,
                            ErrorType::ApiError,
                            "post_ids is only allowed for user_selected history decisions"
                                .to_string(),
                        ));
                    }
                    vec![]
                }
                JournalHistoryDecision::AllNormal => {
                    if payload.post_ids.is_some() {
                        return Err(PpdcError::new(
                            400,
                            ErrorType::ApiError,
                            "post_ids is only allowed for user_selected history decisions"
                                .to_string(),
                        ));
                    }
                    Self::find_history_post_ids_for_policy_with_conn(
                        journal_id,
                        owner_user_id,
                        true,
                        conn,
                    )?
                }
                JournalHistoryDecision::AllIncludingSensitive => {
                    if payload.post_ids.is_some() {
                        return Err(PpdcError::new(
                            400,
                            ErrorType::ApiError,
                            "post_ids is only allowed for user_selected history decisions"
                                .to_string(),
                        ));
                    }
                    Self::find_history_post_ids_for_policy_with_conn(
                        journal_id,
                        owner_user_id,
                        false,
                        conn,
                    )?
                }
                JournalHistoryDecision::UserSelected => {
                    let post_ids = payload.post_ids.ok_or_else(|| {
                        PpdcError::new(
                            400,
                            ErrorType::ApiError,
                            "post_ids is required for user_selected history decisions".to_string(),
                        )
                    })?;
                    if post_ids.is_empty() {
                        return Err(PpdcError::new(
                            400,
                            ErrorType::ApiError,
                            "post_ids must not be empty for user_selected history decisions"
                                .to_string(),
                        ));
                    }
                    Self::validate_selected_history_post_ids_with_conn(
                        journal_id,
                        owner_user_id,
                        post_ids,
                        conn,
                    )?
                }
            };

            PostGrant::upsert_direct_grants_for_posts_with_conn(
                &post_ids,
                owner_user_id,
                policy.grantee_user_id,
                conn,
            )?;

            diesel::update(
                journal_sharing_policies::table.filter(journal_sharing_policies::id.eq(policy_id)),
            )
            .set((
                journal_sharing_policies::history_review_state
                    .eq(JournalHistoryReviewState::Reviewed.to_db()),
                journal_sharing_policies::history_decision.eq(Some(payload.decision.to_db())),
                journal_sharing_policies::history_reviewed_at
                    .eq(Some(chrono::Utc::now().naive_utc())),
                journal_sharing_policies::updated_at.eq(diesel::dsl::now),
            ))
            .execute(conn)?;

            Ok(())
        })?;

        Self::find(policy_id, pool)
    }

    fn find_history_post_ids_for_policy_with_conn(
        journal_id: Uuid,
        owner_user_id: Uuid,
        normal_only: bool,
        conn: &mut PgConnection,
    ) -> Result<Vec<Uuid>, PpdcError> {
        let mut query = posts::table
            .inner_join(traces::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .filter(posts::user_id.eq(owner_user_id))
            .filter(posts::status.ne(PostStatus::Archived.to_db()))
            .filter(traces::journal_id.eq(journal_id))
            .filter(traces::trace_type.eq(TraceType::UserTrace.to_db()))
            .filter(traces::status.ne(TraceStatus::Archived.to_db()))
            .select(posts::id)
            .distinct()
            .into_boxed();

        if normal_only {
            query = query
                .filter(traces::sharing_sensitivity.eq(TraceSharingSensitivity::Normal.to_db()));
        }

        Ok(query.load::<Uuid>(conn)?)
    }

    fn validate_selected_history_post_ids_with_conn(
        journal_id: Uuid,
        owner_user_id: Uuid,
        post_ids: Vec<Uuid>,
        conn: &mut PgConnection,
    ) -> Result<Vec<Uuid>, PpdcError> {
        let mut requested_post_ids = post_ids;
        requested_post_ids.sort_unstable();
        requested_post_ids.dedup();

        let mut valid_post_ids = posts::table
            .inner_join(traces::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .filter(posts::id.eq_any(&requested_post_ids))
            .filter(posts::user_id.eq(owner_user_id))
            .filter(posts::status.ne(PostStatus::Archived.to_db()))
            .filter(traces::journal_id.eq(journal_id))
            .filter(traces::trace_type.eq(TraceType::UserTrace.to_db()))
            .filter(traces::status.ne(TraceStatus::Archived.to_db()))
            .select(posts::id)
            .distinct()
            .load::<Uuid>(conn)?;
        valid_post_ids.sort_unstable();

        if valid_post_ids != requested_post_ids {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Selected posts must be eligible trace-backed posts for this journal".to_string(),
            ));
        }

        Ok(valid_post_ids)
    }

    pub(crate) fn materialize_default_post_grants_for_post(
        post: &Post,
        pool: &DbPool,
    ) -> Result<(), PpdcError> {
        if post.status != PostStatus::Draft {
            return Ok(());
        }

        let Some(trace_id) = post.source_trace_id else {
            return Ok(());
        };

        let trace = Trace::find_full_trace(trace_id, pool)?;
        if trace.trace_type != TraceType::UserTrace || trace.status == TraceStatus::Archived {
            return Ok(());
        }
        let Some(journal_id) = trace.journal_id else {
            return Ok(());
        };

        let mut conn = pool.get()?;
        let policies = journal_sharing_policies::table
            .filter(journal_sharing_policies::journal_id.eq(journal_id))
            .filter(journal_sharing_policies::status.eq(JournalSharingPolicyStatus::Active.to_db()))
            .filter(journal_sharing_policies::default_future_access_enabled.eq(true))
            .select((
                journal_sharing_policies::grantee_user_id,
                journal_sharing_policies::owner_user_id,
            ))
            .load::<(Uuid, Uuid)>(&mut conn)?;

        conn.transaction::<(), PpdcError, _>(|conn| {
            for (grantee_user_id, owner_user_id) in policies {
                if !is_follow_accepted_with_conn(grantee_user_id, owner_user_id, conn)? {
                    continue;
                }
                PostGrant::upsert_direct_grants_for_posts_with_conn(
                    &[post.id],
                    owner_user_id,
                    grantee_user_id,
                    conn,
                )?;
            }
            Ok(())
        })?;

        Ok(())
    }

    pub(crate) fn create_policies_for_new_follower_pair_with_conn(
        owner_user_id: Uuid,
        follower_user_id: Uuid,
        conn: &mut PgConnection,
    ) -> Result<(), PpdcError> {
        let journals = journals::table
            .filter(journals::user_id.eq(owner_user_id))
            .filter(journals::status.eq(JournalStatus::Active.to_db()))
            .filter(journals::is_encrypted.eq(false))
            .select((journals::id, journals::sharing_mode))
            .load::<(Uuid, String)>(conn)?;

        for (journal_id, sharing_mode_raw) in journals {
            let sharing_mode = JournalSharingMode::from_db(&sharing_mode_raw);
            let Some((status, default_future_access_enabled, history_review_state)) =
                default_policy_values_for_sharing_mode(sharing_mode)
            else {
                continue;
            };

            create_policy_if_absent_with_conn(
                journal_id,
                owner_user_id,
                follower_user_id,
                status,
                default_future_access_enabled,
                history_review_state,
                conn,
            )?;
        }

        Ok(())
    }

    pub(crate) fn create_missing_policies_for_existing_followers(
        journal: &Journal,
        pool: &DbPool,
    ) -> Result<(), PpdcError> {
        if journal.status != JournalStatus::Active || journal.is_encrypted {
            return Ok(());
        }
        let Some((status, default_future_access_enabled, history_review_state)) =
            default_policy_values_for_sharing_mode(journal.sharing_mode)
        else {
            return Ok(());
        };

        let mut conn = pool.get()?;
        conn.transaction::<(), PpdcError, _>(|conn| {
            let follower_ids =
                find_accepted_follower_ids_for_user_with_conn(journal.user_id, conn)?;
            for follower_user_id in follower_ids {
                create_policy_if_absent_with_conn(
                    journal.id,
                    journal.user_id,
                    follower_user_id,
                    status,
                    default_future_access_enabled,
                    history_review_state,
                    conn,
                )?;
            }
            Ok(())
        })
    }
}
