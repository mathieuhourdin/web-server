use diesel::prelude::*;
use diesel::sql_types::{Array, BigInt, Bool, Nullable, Text, Timestamp, Uuid as SqlUuid};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::model::*;
use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}
#[derive(QueryableByName)]
struct Id {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
}
#[derive(QueryableByName)]
struct Changed {
    #[diesel(sql_type = Bool)]
    changed: bool,
}

#[derive(QueryableByName)]
struct AudienceTraceCount {
    #[diesel(sql_type = SqlUuid)]
    audience_id: Uuid,
    #[diesel(sql_type = BigInt)]
    trace_count: i64,
}

const ELIGIBLE_TRACE_PUBLICATION: &str =
    "t.trace_type='USER_TRACE' AND trace_publication_eligible(p.id,p.user_id)";

/// Hydrate the returned page in one grouped query, not one query per audience.
pub fn with_trace_counts(
    audiences: Vec<Audience>,
    pool: &DbPool,
) -> Result<Vec<AudienceView>, PpdcError> {
    if audiences.is_empty() {
        return Ok(Vec::new());
    }
    let ids = audiences.iter().map(|a| a.id).collect::<Vec<_>>();
    let rows = diesel::sql_query(format!(
        "SELECT pa.audience_id,count(*)::bigint AS trace_count
         FROM post_audiences pa JOIN audiences a ON a.id=pa.audience_id
         JOIN posts p ON p.id=pa.post_id JOIN traces t ON t.id=p.source_trace_id
         WHERE pa.audience_id=ANY($1) AND p.user_id=a.owner_user_id AND {ELIGIBLE_TRACE_PUBLICATION}
         GROUP BY pa.audience_id"
    ))
    .bind::<Array<SqlUuid>, _>(ids)
    .load::<AudienceTraceCount>(&mut pool.get()?)?;
    let counts = rows
        .into_iter()
        .map(|row| (row.audience_id, row.trace_count))
        .collect::<HashMap<_, _>>();
    Ok(audiences
        .into_iter()
        .map(|audience| AudienceView {
            trace_count: counts.get(&audience.id).copied().unwrap_or(0),
            audience,
        })
        .collect())
}

fn invalid(message: &str) -> PpdcError {
    PpdcError::new(400, ErrorType::ApiError, message.to_string())
}

pub fn validate_name(name: &str) -> Result<&str, PpdcError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(invalid("Audience name must contain 1 to 100 characters"));
    }
    Ok(name)
}

pub fn owned(conn: &mut PgConnection, owner: Uuid, aid: Uuid) -> Result<Audience, PpdcError> {
    diesel::sql_query("SELECT * FROM audiences WHERE id=$1 AND owner_user_id=$2 FOR UPDATE")
        .bind::<SqlUuid, _>(aid)
        .bind::<SqlUuid, _>(owner)
        .get_result(conn)
        .optional()?
        .ok_or_else(|| PpdcError::new(404, ErrorType::ApiError, "Audience not found".to_string()))
}

fn writable(audience: &Audience) -> Result<(), PpdcError> {
    if audience.archived_at.is_some() {
        return Err(invalid("Archived audiences cannot be modified"));
    }
    Ok(())
}

pub fn member(conn: &mut PgConnection, aid: Uuid, uid: Uuid) -> Result<AudienceMember, PpdcError> {
    let row =
        diesel::sql_query("SELECT * FROM audience_members WHERE audience_id=$1 AND user_id=$2")
            .bind::<SqlUuid, _>(aid)
            .bind::<SqlUuid, _>(uid)
            .get_result::<AudienceMember>(conn)
            .optional()?
            .ok_or_else(|| {
                PpdcError::new(
                    404,
                    ErrorType::ApiError,
                    "Audience member not found".to_string(),
                )
            })?;
    Ok(row.normalize())
}

fn follower(conn: &mut PgConnection, owner: Uuid, uid: Uuid) -> Result<(), PpdcError> {
    if owner == uid {
        return Err(invalid("An audience member must be an accepted follower"));
    }
    let rows = diesel::sql_query("SELECT r.id FROM relationships r JOIN users u ON u.id=r.requester_user_id
        WHERE r.requester_user_id=$1 AND r.target_user_id=$2 AND r.relationship_type='FOLLOW' AND r.status='ACCEPTED'
          AND u.principal_type='HUMAN' AND u.is_platform_user
          AND NOT EXISTS (SELECT 1 FROM user_blocks b WHERE
            (b.blocker_user_id=$1 AND b.blocked_user_id=$2) OR (b.blocker_user_id=$2 AND b.blocked_user_id=$1))
        FOR SHARE OF r")
        .bind::<SqlUuid,_>(uid).bind::<SqlUuid,_>(owner).load::<Id>(conn)?;
    if rows.is_empty() {
        return Err(PpdcError::new(
            403,
            ErrorType::ApiError,
            "Audience members must be unblocked accepted followers".to_string(),
        ));
    }
    Ok(())
}

pub fn list(
    owner: Uuid,
    archived: bool,
    offset: i64,
    limit: i64,
    pool: &DbPool,
) -> Result<(Vec<Audience>, i64), PpdcError> {
    let mut conn = pool.get()?;
    let count=diesel::sql_query("SELECT count(*)::bigint AS count FROM audiences WHERE owner_user_id=$1 AND ($2 OR archived_at IS NULL)")
        .bind::<SqlUuid,_>(owner).bind::<Bool,_>(archived).get_result::<Count>(&mut conn)?.count;
    let items=diesel::sql_query("SELECT * FROM audiences WHERE owner_user_id=$1 AND ($2 OR archived_at IS NULL) ORDER BY created_at,id OFFSET $3 LIMIT $4")
        .bind::<SqlUuid,_>(owner).bind::<Bool,_>(archived).bind::<BigInt,_>(offset).bind::<BigInt,_>(limit).load(&mut conn)?;
    Ok((items, count))
}

pub fn has_journal_defaults(jid: Uuid, pool: &DbPool) -> Result<bool, PpdcError> {
    let mut conn = pool.get()?;
    Ok(diesel::sql_query(
        "SELECT count(*)::bigint AS count FROM journal_default_audiences WHERE journal_id=$1",
    )
    .bind::<SqlUuid, _>(jid)
    .get_result::<Count>(&mut conn)?
    .count
        > 0)
}

// Preserve the existing all-followers behavior of shared journals, but write
// audiences only. Named, manually created audiences are never auto-populated.
fn shared_journal_audience(
    conn: &mut PgConnection,
    jid: Uuid,
    owner: Uuid,
) -> Result<Option<Uuid>, PpdcError> {
    let existing =
        diesel::sql_query("SELECT id FROM audiences WHERE source_journal_id=$1 FOR UPDATE")
            .bind::<SqlUuid, _>(jid)
            .get_result::<Id>(conn)
            .optional()?;
    if let Some(existing) = existing {
        let active = diesel::sql_query(
            "SELECT a.id FROM audiences a JOIN journal_default_audiences d ON d.audience_id=a.id
            WHERE a.id=$1 AND d.journal_id=$2 AND a.archived_at IS NULL",
        )
        .bind::<SqlUuid, _>(existing.id)
        .bind::<SqlUuid, _>(jid)
        .get_result::<Id>(conn)
        .optional()?;
        return Ok(active.map(|a| a.id));
    }
    let aid = diesel::sql_query("INSERT INTO audiences(owner_user_id,name,source_journal_id)
        SELECT user_id,LEFT(COALESCE(NULLIF(btrim(title),''),'Journal'),100),id FROM journals WHERE id=$1 AND user_id=$2
        ON CONFLICT(source_journal_id) DO NOTHING RETURNING id")
        .bind::<SqlUuid,_>(jid).bind::<SqlUuid,_>(owner).get_result::<Id>(conn).optional()?;
    let Some(aid) = aid else {
        return Ok(diesel::sql_query("SELECT a.id FROM audiences a JOIN journal_default_audiences d ON d.audience_id=a.id
            WHERE a.source_journal_id=$1 AND d.journal_id=$1 AND a.owner_user_id=$2 AND a.archived_at IS NULL FOR UPDATE OF a")
            .bind::<SqlUuid,_>(jid).bind::<SqlUuid,_>(owner).get_result::<Id>(conn).optional()?.map(|a| a.id));
    };
    diesel::sql_query(
        "INSERT INTO journal_default_audiences(journal_id,audience_id) VALUES($1,$2)",
    )
    .bind::<SqlUuid, _>(jid)
    .bind::<SqlUuid, _>(aid.id)
    .execute(conn)?;
    diesel::sql_query("INSERT INTO post_audiences(post_id,audience_id,is_default)
        SELECT p.id,$2,TRUE FROM posts p JOIN traces t ON t.id=p.source_trace_id
        WHERE t.journal_id=$1 AND p.user_id=$3 AND t.trace_type='USER_TRACE' ON CONFLICT DO NOTHING")
        .bind::<SqlUuid,_>(jid).bind::<SqlUuid,_>(aid.id).bind::<SqlUuid,_>(owner).execute(conn)?;
    Ok(Some(aid.id))
}

fn auto_add_member(
    conn: &mut PgConnection,
    owner: Uuid,
    aid: Uuid,
    reader: Uuid,
) -> Result<bool, PpdcError> {
    // Existing paused/revoked memberships are decisions, not missing data.
    let inserted = diesel::sql_query("INSERT INTO audience_members(audience_id,user_id)
        SELECT $1,u.id FROM users u WHERE u.id=$2 AND u.principal_type='HUMAN' AND u.is_platform_user
        AND EXISTS(SELECT 1 FROM relationships r WHERE r.requester_user_id=u.id AND r.target_user_id=$3
            AND r.relationship_type='FOLLOW' AND r.status='ACCEPTED')
        AND NOT EXISTS(SELECT 1 FROM user_blocks b WHERE (b.blocker_user_id=$3 AND b.blocked_user_id=u.id)
            OR (b.blocker_user_id=u.id AND b.blocked_user_id=$3))
        ON CONFLICT(audience_id,user_id) DO NOTHING")
        .bind::<SqlUuid,_>(aid).bind::<SqlUuid,_>(reader).bind::<SqlUuid,_>(owner).execute(conn)?;
    if inserted == 0 {
        return Ok(false);
    }
    let inserted_member = member(conn, aid, reader)?;
    if history_ids(conn, owner, aid, &inserted_member, false)?.is_empty() {
        mark_review(conn, aid, reader, "NONE")?;
        return Ok(false);
    }
    Ok(true)
}

pub(crate) fn create_members_for_new_follower_pair_with_conn(
    owner: Uuid,
    reader: Uuid,
    conn: &mut PgConnection,
) -> Result<Vec<(Uuid, Uuid, Uuid)>, PpdcError> {
    let journals = diesel::sql_query(
        "SELECT id FROM journals WHERE user_id=$1 AND status='ACTIVE'
        AND NOT is_encrypted AND sharing_mode='SHARED' ORDER BY id",
    )
    .bind::<SqlUuid, _>(owner)
    .load::<Id>(conn)?;
    let mut pending = Vec::new();
    for journal in journals {
        if let Some(aid) = shared_journal_audience(conn, journal.id, owner)? {
            if auto_add_member(conn, owner, aid, reader)? {
                pending.push((owner, aid, reader));
            }
        }
    }
    Ok(pending)
}

pub(crate) fn create_missing_members_for_existing_followers(
    journal: &crate::entities_v2::journal::Journal,
    pool: &DbPool,
) -> Result<Vec<(Uuid, Uuid, Uuid)>, PpdcError> {
    use crate::entities_v2::journal::{JournalSharingMode, JournalStatus};
    if journal.status != JournalStatus::Active
        || journal.is_encrypted
        || journal.sharing_mode != JournalSharingMode::Shared
    {
        return Ok(Vec::new());
    }
    let mut conn = pool.get()?;
    conn.transaction(|conn| {
        let Some(aid) = shared_journal_audience(conn, journal.id, journal.user_id)? else {
            return Ok(Vec::new());
        };
        let followers = diesel::sql_query(
            "SELECT requester_user_id AS id FROM relationships WHERE target_user_id=$1
            AND relationship_type='FOLLOW' AND status='ACCEPTED' ORDER BY requester_user_id",
        )
        .bind::<SqlUuid, _>(journal.user_id)
        .load::<Id>(conn)?;
        let mut pending = Vec::new();
        for reader in followers {
            if auto_add_member(conn, journal.user_id, aid, reader.id)? {
                pending.push((journal.user_id, aid, reader.id));
            }
        }
        Ok(pending)
    })
}

pub fn create(owner: Uuid, name: &str, pool: &DbPool) -> Result<Audience, PpdcError> {
    Ok(
        diesel::sql_query("INSERT INTO audiences(owner_user_id,name) VALUES($1,$2) RETURNING *")
            .bind::<SqlUuid, _>(owner)
            .bind::<Text, _>(validate_name(name)?)
            .get_result(&mut pool.get()?)?,
    )
}

pub fn rename(owner: Uuid, aid: Uuid, name: &str, pool: &DbPool) -> Result<Audience, PpdcError> {
    let mut conn = pool.get()?;
    conn.transaction(|conn| {
        writable(&owned(conn, owner, aid)?)?;
        Ok(diesel::sql_query(
            "UPDATE audiences SET name=$2,updated_at=NOW() WHERE id=$1 RETURNING *",
        )
        .bind::<SqlUuid, _>(aid)
        .bind::<Text, _>(validate_name(name)?)
        .get_result(conn)?)
    })
}

pub fn archive(
    owner: Uuid,
    aid: Uuid,
    revoke: bool,
    pool: &DbPool,
) -> Result<AudienceChange, PpdcError> {
    let mut conn = pool.get()?;
    conn.transaction(|conn| {
        owned(conn,owner,aid)?;
        diesel::sql_query("UPDATE audience_members SET future_access_enabled=FALSE,updated_at=NOW() WHERE audience_id=$1")
            .bind::<SqlUuid,_>(aid).execute(conn)?;
        diesel::sql_query("DELETE FROM journal_default_audiences WHERE audience_id=$1")
            .bind::<SqlUuid,_>(aid).execute(conn)?;
        let revoked=if revoke { diesel::sql_query("SELECT revoke_audience_grant_sources($1,NULL,NULL) AS count")
            .bind::<SqlUuid,_>(aid).get_result::<Count>(conn)?.count } else {0};
        let audience=diesel::sql_query("UPDATE audiences SET archived_at=COALESCE(archived_at,NOW()),updated_at=NOW() WHERE id=$1 RETURNING *")
            .bind::<SqlUuid,_>(aid).get_result(conn)?;
        Ok(AudienceChange{audience,newly_shared_grant_count:0,revoked_grant_count:revoked})
    })
}

pub fn list_members(
    owner: Uuid,
    aid: Uuid,
    offset: i64,
    limit: i64,
    pool: &DbPool,
) -> Result<(Vec<AudienceMember>, i64), PpdcError> {
    let mut conn = pool.get()?;
    owned(&mut conn, owner, aid)?;
    let count = diesel::sql_query(
        "SELECT count(*)::bigint AS count FROM audience_members WHERE audience_id=$1",
    )
    .bind::<SqlUuid, _>(aid)
    .get_result::<Count>(&mut conn)?
    .count;
    let items=diesel::sql_query("SELECT * FROM audience_members WHERE audience_id=$1 ORDER BY created_at,id OFFSET $2 LIMIT $3")
        .bind::<SqlUuid,_>(aid).bind::<BigInt,_>(offset).bind::<BigInt,_>(limit).load::<AudienceMember>(&mut conn)?;
    Ok((
        items.into_iter().map(AudienceMember::normalize).collect(),
        count,
    ))
}

pub fn add_member(
    owner: Uuid,
    aid: Uuid,
    uid: Uuid,
    future: bool,
    pool: &DbPool,
) -> Result<(AudienceMember, bool), PpdcError> {
    let mut conn = pool.get()?;
    conn.transaction(|conn| {
        writable(&owned(conn,owner,aid)?)?; follower(conn,owner,uid)?;
        let previous=member(conn,aid,uid).optional_member()?;
        let reset=previous.as_ref().map(|m| m.status!="active" || (!m.future_access_enabled && future)).unwrap_or(true);
        diesel::sql_query("INSERT INTO audience_members(audience_id,user_id,future_access_enabled)
            VALUES($1,$2,$3)
            ON CONFLICT(audience_id,user_id) DO UPDATE SET status='ACTIVE',future_access_enabled=EXCLUDED.future_access_enabled,
                history_cutoff_at=CASE WHEN $4 THEN NOW() ELSE audience_members.history_cutoff_at END,
                history_review_state=CASE WHEN $4 THEN 'UNREVIEWED' ELSE audience_members.history_review_state END,
                history_decision=CASE WHEN $4 THEN NULL ELSE audience_members.history_decision END,
                history_reviewed_at=CASE WHEN $4 THEN NULL ELSE audience_members.history_reviewed_at END,updated_at=NOW()")
            .bind::<SqlUuid,_>(aid).bind::<SqlUuid,_>(uid).bind::<Bool,_>(future).bind::<Bool,_>(reset).execute(conn)?;
        let mut result=member(conn,aid,uid)?;
        if reset && history_ids(conn,owner,aid,&result,false)?.is_empty() {
            mark_review(conn,aid,uid,"NONE")?;
            result=member(conn,aid,uid)?;
        }
        let notify=reset && result.history_review_state=="unreviewed";
        Ok((result,notify))
    })
}

// Keep optional member lookup distinct from authorization/database failures.
trait OptionalMember {
    fn optional_member(self) -> Result<Option<AudienceMember>, PpdcError>;
}
impl OptionalMember for Result<AudienceMember, PpdcError> {
    fn optional_member(self) -> Result<Option<AudienceMember>, PpdcError> {
        match self {
            Ok(member) => Ok(Some(member)),
            Err(err) if err.status_code == 404 => Ok(None),
            Err(err) => Err(err),
        }
    }
}

pub fn change_future(
    owner: Uuid,
    aid: Uuid,
    uid: Uuid,
    enabled: bool,
    pool: &DbPool,
) -> Result<AudienceMember, PpdcError> {
    let mut conn = pool.get()?;
    conn.transaction(|conn| {
        writable(&owned(conn, owner, aid)?)?;
        let current = member(conn, aid, uid)?;
        if enabled && current.status != "active" {
            return Err(invalid(
                "Use PUT member to reactivate an inactive membership",
            ));
        }
        if enabled {
            follower(conn, owner, uid)?;
        }
        diesel::sql_query(
            "UPDATE audience_members SET future_access_enabled=$3,
            status=CASE WHEN status='SUGGESTED' AND NOT $3 THEN 'ACTIVE' ELSE status END,
            updated_at=NOW() WHERE audience_id=$1 AND user_id=$2",
        )
        .bind::<SqlUuid, _>(aid)
        .bind::<SqlUuid, _>(uid)
        .bind::<Bool, _>(enabled)
        .execute(conn)?;
        member(conn, aid, uid)
    })
}

pub fn restart_history_review(
    owner: Uuid,
    aid: Uuid,
    uid: Uuid,
    pool: &DbPool,
) -> Result<AudienceMember, PpdcError> {
    let mut conn = pool.get()?;
    conn.transaction(|conn| {
        writable(&owned(conn,owner,aid)?)?; follower(conn,owner,uid)?;
        let current=member(conn,aid,uid)?;
        if current.status!="active" && current.status!="suggested" { return Err(invalid("Member must be active or suggested")); }
        diesel::sql_query("UPDATE audience_members SET history_cutoff_at=NOW(),history_review_state='UNREVIEWED',
            history_decision=NULL,history_reviewed_at=NULL,updated_at=NOW() WHERE audience_id=$1 AND user_id=$2")
            .bind::<SqlUuid,_>(aid).bind::<SqlUuid,_>(uid).execute(conn)?;
        let result=member(conn,aid,uid)?;
        if history_ids(conn,owner,aid,&result,false)?.is_empty() {mark_review(conn,aid,uid,"NONE")?;}
        member(conn,aid,uid)
    })
}

pub fn remove_member(
    owner: Uuid,
    aid: Uuid,
    uid: Uuid,
    stop_future: bool,
    revoke: bool,
    pool: &DbPool,
) -> Result<MemberChange, PpdcError> {
    if !stop_future && !revoke {
        return Err(invalid("Select stop_future, revoke_history, or both"));
    }
    let mut conn = pool.get()?;
    conn.transaction(|conn| {
        owned(conn,owner,aid)?; member(conn,aid,uid)?;
        if stop_future { diesel::sql_query("UPDATE audience_members SET future_access_enabled=FALSE,
            status=CASE WHEN status='SUGGESTED' THEN 'ACTIVE' ELSE status END,updated_at=NOW() WHERE audience_id=$1 AND user_id=$2")
            .bind::<SqlUuid,_>(aid).bind::<SqlUuid,_>(uid).execute(conn)?; }
        let revoked=if revoke { diesel::sql_query("SELECT revoke_audience_grant_sources($1,$2,NULL) AS count")
            .bind::<SqlUuid,_>(aid).bind::<SqlUuid,_>(uid).get_result::<Count>(conn)?.count } else {0};
        if revoke { mark_review(conn,aid,uid,"NONE")?; }
        Ok(MemberChange{member:member(conn,aid,uid)?,revoked_post_count:revoked})
    })
}

fn history_ids(
    conn: &mut PgConnection,
    owner: Uuid,
    aid: Uuid,
    member: &AudienceMember,
    normal: bool,
) -> Result<Vec<Uuid>, PpdcError> {
    let rows=diesel::sql_query("SELECT p.id FROM post_audiences pa JOIN posts p ON p.id=pa.post_id JOIN traces t ON t.id=p.source_trace_id
        WHERE pa.audience_id=$1 AND p.user_id=$2 AND t.trace_type='USER_TRACE'
          AND trace_publication_eligible(p.id,p.user_id)
          AND COALESCE(p.publishing_date,p.created_at)<=$3 AND pa.created_at<=$3
          AND (NOT $4 OR t.sharing_sensitivity='NORMAL') ORDER BY p.id")
        .bind::<SqlUuid,_>(aid).bind::<SqlUuid,_>(owner).bind::<Timestamp,_>(member.history_cutoff_at).bind::<Bool,_>(normal).load::<Id>(conn)?;
    Ok(rows.into_iter().map(|row| row.id).collect())
}

fn mark_review(
    conn: &mut PgConnection,
    aid: Uuid,
    uid: Uuid,
    decision: &str,
) -> Result<(), PpdcError> {
    diesel::sql_query(
        "UPDATE audience_members SET history_review_state='REVIEWED',history_decision=$3,
        history_reviewed_at=NOW(),updated_at=NOW() WHERE audience_id=$1 AND user_id=$2",
    )
    .bind::<SqlUuid, _>(aid)
    .bind::<SqlUuid, _>(uid)
    .bind::<Text, _>(decision)
    .execute(conn)?;
    Ok(())
}

pub fn apply_history(
    owner: Uuid,
    aid: Uuid,
    uid: Uuid,
    payload: HistoryDecisionDto,
    pool: &DbPool,
) -> Result<HistoryDecisionResult, PpdcError> {
    use JournalHistoryDecision::*;
    if payload.decision != UserSelected && payload.post_ids.is_some() {
        return Err(invalid("post_ids is only allowed for user_selected"));
    }
    let mut conn = pool.get()?;
    conn.transaction(|conn| {
        writable(&owned(conn,owner,aid)?)?; follower(conn,owner,uid)?;
        let current=member(conn,aid,uid)?;
        if current.status!="active" && current.status!="suggested" { return Err(invalid("Member must be active or suggested")); }
        if current.history_review_state=="reviewed" {
            if current.history_decision.as_deref()!=Some(&payload.decision.to_db().to_lowercase()) {
                return Err(PpdcError::new(409,ErrorType::ApiError,"History review has already been decided".to_string()));
            }
            return Ok(HistoryDecisionResult{member:current,newly_shared_post_count:0});
        }
        let ids=match payload.decision {
            None=>Vec::new(),
            AllNormal=>history_ids(conn,owner,aid,&current,true)?,
            AllIncludingSensitive=>history_ids(conn,owner,aid,&current,false)?,
            UserSelected=>{
                let ids=payload.post_ids.ok_or_else(||invalid("post_ids is required for user_selected"))?;
                if ids.is_empty() || ids.len()>1000 { return Err(invalid("Select between 1 and 1000 post_ids")); }
                let valid=history_ids(conn,owner,aid,&current,false)?.into_iter().collect::<HashSet<_>>();
                if ids.iter().any(|id|!valid.contains(id)) { return Err(invalid("Selected posts must belong to this audience and precede the history cutoff")); }
                ids.into_iter().collect::<HashSet<_>>().into_iter().collect()
            }
        };
        let mut changed=0;
        for pid in ids {
            if diesel::sql_query("SELECT grant_audience_post($1,$2,$3,TRUE) AS changed")
                .bind::<SqlUuid,_>(aid).bind::<SqlUuid,_>(pid).bind::<SqlUuid,_>(uid).get_result::<Changed>(conn)?.changed { changed+=1; }
        }
        mark_review(conn,aid,uid,payload.decision.to_db())?;
        Ok(HistoryDecisionResult{member:member(conn,aid,uid)?,newly_shared_post_count:changed})
    })
}

pub fn pending(
    owner: Uuid,
    offset: i64,
    limit: i64,
    pool: &DbPool,
) -> Result<(Vec<AudienceMember>, i64), PpdcError> {
    let mut conn = pool.get()?;
    let count=diesel::sql_query("SELECT count(*)::bigint AS count FROM audience_members m JOIN audiences a ON a.id=m.audience_id
        WHERE a.owner_user_id=$1 AND a.archived_at IS NULL AND
            (m.status='SUGGESTED' OR (m.status='ACTIVE' AND m.history_review_state IN ('NOT_STARTED','UNREVIEWED')))")
        .bind::<SqlUuid,_>(owner).get_result::<Count>(&mut conn)?.count;
    let items = diesel::sql_query(
        "SELECT m.* FROM audience_members m JOIN audiences a ON a.id=m.audience_id
        WHERE a.owner_user_id=$1 AND a.archived_at IS NULL AND
            (m.status='SUGGESTED' OR (m.status='ACTIVE' AND m.history_review_state IN ('NOT_STARTED','UNREVIEWED')))
        ORDER BY m.created_at,m.id OFFSET $2 LIMIT $3",
    )
    .bind::<SqlUuid, _>(owner)
    .bind::<BigInt, _>(offset)
    .bind::<BigInt, _>(limit)
    .load::<AudienceMember>(&mut conn)?;
    Ok((
        items.into_iter().map(AudienceMember::normalize).collect(),
        count,
    ))
}

pub fn list_posts(
    owner: Uuid,
    aid: Uuid,
    review_user: Option<Uuid>,
    offset: i64,
    limit: i64,
    pool: &DbPool,
) -> Result<(Vec<AudiencePost>, i64), PpdcError> {
    let mut conn = pool.get()?;
    owned(&mut conn, owner, aid)?;
    let cutoff = review_user
        .map(|uid| member(&mut conn, aid, uid).map(|m| m.history_cutoff_at))
        .transpose()?;
    let reader = review_user.unwrap_or(owner);
    let predicate=format!(" FROM post_audiences pa JOIN posts p ON p.id=pa.post_id JOIN traces t ON t.id=p.source_trace_id
        WHERE pa.audience_id=$1 AND p.user_id=$2 AND {ELIGIBLE_TRACE_PUBLICATION}
          AND ($3::timestamp IS NULL OR (COALESCE(p.publishing_date,p.created_at)<=$3 AND pa.created_at<=$3))");
    let count = diesel::sql_query(format!("SELECT count(*)::bigint AS count{predicate}"))
        .bind::<SqlUuid, _>(aid)
        .bind::<SqlUuid, _>(owner)
        .bind::<Nullable<Timestamp>, _>(cutoff)
        .get_result::<Count>(&mut conn)?
        .count;
    let items=diesel::sql_query(format!("SELECT p.id AS post_id,t.id AS trace_id,t.journal_id,t.user_id AS author_user_id,t.title,t.content,
        lower(t.sharing_sensitivity) AS sharing_sensitivity,p.publishing_date,trace_publication_readable(p.id,$4) AS has_access{predicate}
        ORDER BY COALESCE(p.publishing_date,p.created_at) DESC,p.id DESC OFFSET $5 LIMIT $6"))
        .bind::<SqlUuid,_>(aid).bind::<SqlUuid,_>(owner).bind::<Nullable<Timestamp>,_>(cutoff).bind::<SqlUuid,_>(reader)
        .bind::<BigInt,_>(offset).bind::<BigInt,_>(limit).load(&mut conn)?;
    Ok((items, count))
}

fn publication(conn: &mut PgConnection, owner: Uuid, pid: Uuid) -> Result<(), PpdcError> {
    let ids = diesel::sql_query(
        "SELECT p.id FROM posts p JOIN traces t ON t.id=p.source_trace_id
        WHERE p.id=$1 AND p.user_id=$2 AND t.trace_type='USER_TRACE' FOR UPDATE OF p",
    )
    .bind::<SqlUuid, _>(pid)
    .bind::<SqlUuid, _>(owner)
    .load::<Id>(conn)?;
    if ids.is_empty() {
        return Err(PpdcError::unauthorized());
    }
    Ok(())
}

pub fn attach(
    owner: Uuid,
    aid: Uuid,
    pid: Uuid,
    pool: &DbPool,
) -> Result<AudienceChange, PpdcError> {
    let mut conn = pool.get()?;
    conn.transaction(|conn| {
        let audience=owned(conn,owner,aid)?; writable(&audience)?; publication(conn,owner,pid)?;
        let inserted=diesel::sql_query("INSERT INTO post_audiences(post_id,audience_id) VALUES($1,$2)
            ON CONFLICT(post_id,audience_id) DO NOTHING")
            .bind::<SqlUuid,_>(pid).bind::<SqlUuid,_>(aid).execute(conn)?;
        // Repeated attachment is idempotent and never restores withdrawn grants.
        let mut changed=0;
        {
            let users=diesel::sql_query("SELECT user_id AS id FROM audience_members WHERE audience_id=$1 AND status='ACTIVE'
                AND future_access_enabled ORDER BY user_id").bind::<SqlUuid,_>(aid).load::<Id>(conn)?;
            for user in users { if diesel::sql_query("SELECT grant_audience_post($1,$2,$3,$4) AS changed")
                .bind::<SqlUuid,_>(aid).bind::<SqlUuid,_>(pid).bind::<SqlUuid,_>(user.id).bind::<Bool,_>(inserted>0).get_result::<Changed>(conn)?.changed { changed+=1; } }
        }
        // Deprecated migration metadata is not used to resolve recipients.
        diesel::sql_query("UPDATE post_audiences SET is_default=FALSE WHERE post_id=$1 AND audience_id=$2")
            .bind::<SqlUuid,_>(pid).bind::<SqlUuid,_>(aid).execute(conn)?;
        Ok(AudienceChange{audience,newly_shared_grant_count:changed,revoked_grant_count:0})
    })
}

pub fn detach(
    owner: Uuid,
    aid: Uuid,
    pid: Uuid,
    revoke: bool,
    pool: &DbPool,
) -> Result<AudienceChange, PpdcError> {
    let mut conn = pool.get()?;
    conn.transaction(|conn| {
        let audience = owned(conn, owner, aid)?;
        publication(conn, owner, pid)?;
        diesel::sql_query("DELETE FROM post_audiences WHERE post_id=$1 AND audience_id=$2")
            .bind::<SqlUuid, _>(pid)
            .bind::<SqlUuid, _>(aid)
            .execute(conn)?;
        let revoked = if revoke {
            diesel::sql_query("SELECT revoke_audience_grant_sources($2,NULL,$1) AS count")
                .bind::<SqlUuid, _>(pid)
                .bind::<SqlUuid, _>(aid)
                .get_result::<Count>(conn)?
                .count
        } else {
            0
        };
        Ok(AudienceChange {
            audience,
            newly_shared_grant_count: 0,
            revoked_grant_count: revoked,
        })
    })
}

pub fn post_audiences(owner: Uuid, pid: Uuid, pool: &DbPool) -> Result<Vec<Audience>, PpdcError> {
    let mut conn = pool.get()?;
    publication(&mut conn, owner, pid)?;
    Ok(diesel::sql_query("SELECT a.* FROM audiences a JOIN post_audiences pa ON pa.audience_id=a.id WHERE pa.post_id=$1 ORDER BY a.created_at,a.id")
        .bind::<SqlUuid,_>(pid).load(&mut conn)?)
}

fn journal(conn: &mut PgConnection, owner: Uuid, jid: Uuid) -> Result<(), PpdcError> {
    let ids = diesel::sql_query(
        "SELECT id FROM journals WHERE id=$1 AND user_id=$2 AND NOT is_encrypted FOR UPDATE",
    )
    .bind::<SqlUuid, _>(jid)
    .bind::<SqlUuid, _>(owner)
    .load::<Id>(conn)?;
    if ids.is_empty() {
        return Err(PpdcError::unauthorized());
    }
    Ok(())
}

pub fn default_audiences(
    owner: Uuid,
    jid: Uuid,
    pool: &DbPool,
) -> Result<Vec<Audience>, PpdcError> {
    let mut conn = pool.get()?;
    journal(&mut conn, owner, jid)?;
    Ok(diesel::sql_query("SELECT a.* FROM audiences a JOIN journal_default_audiences d ON d.audience_id=a.id WHERE d.journal_id=$1 ORDER BY a.created_at,a.id")
        .bind::<SqlUuid,_>(jid).load(&mut conn)?)
}

pub fn replace_defaults(
    owner: Uuid,
    jid: Uuid,
    ids: &[Uuid],
    pool: &DbPool,
) -> Result<Vec<Audience>, PpdcError> {
    if ids.len() > 100 {
        return Err(invalid("At most 100 default audiences are allowed"));
    }
    let mut conn = pool.get()?;
    conn.transaction(|conn| {
        journal(conn,owner,jid)?;
        let mut ids=ids.to_vec(); ids.sort_unstable(); ids.dedup();
        for aid in &ids { writable(&owned(conn,owner,*aid)?)?; }
        diesel::sql_query("DELETE FROM journal_default_audiences WHERE journal_id=$1").bind::<SqlUuid,_>(jid).execute(conn)?;
        diesel::sql_query("INSERT INTO journal_default_audiences(journal_id,audience_id) SELECT $1,unnest($2::uuid[])")
            .bind::<SqlUuid,_>(jid).bind::<Array<SqlUuid>,_>(&ids).execute(conn)?;
        Ok(diesel::sql_query("SELECT * FROM audiences WHERE id=ANY($1) ORDER BY created_at,id").bind::<Array<SqlUuid>,_>(&ids).load(conn)?)
    })
}
