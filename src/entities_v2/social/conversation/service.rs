use super::{model::*, notifications::MemberAdded};
use crate::{
    db::DbPool,
    entities_v2::{
        asset::Asset,
        error::{ErrorType, PpdcError},
        message::{Message, MessageReaction},
        post::Post,
        post_grant::PostGrant,
        trace::Trace,
        user::{User, UserPrincipalType, UserPublicResponse},
        user_block::UserBlock,
    },
    pagination::{PaginatedResponse, ValidatedPagination},
};
use diesel::{
    prelude::*,
    sql_types::{Array, BigInt, Bool, Nullable, Text, Timestamp, Uuid as SqlUuid},
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub(super) fn invalid(s: &str) -> PpdcError {
    PpdcError::new(400, ErrorType::ApiError, s.into())
}
fn forbidden() -> PpdcError {
    PpdcError::new(
        403,
        ErrorType::ApiError,
        "Conversation access denied".into(),
    )
}
#[derive(QueryableByName)]
struct Id {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
}
#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type=BigInt)]
    count: i64,
}
#[derive(QueryableByName)]
struct Flag {
    #[diesel(sql_type=Bool)]
    value: bool,
}

pub fn ensure_message_readable(message: Uuid, user: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
    ensure_message_readable_on_connection(message, user, &mut *pool.get()?)
}
pub(crate) fn ensure_message_readable_on_connection(
    message: Uuid,
    user: Uuid,
    conn: &mut PgConnection,
) -> Result<(), PpdcError> {
    // Retain the lock in write transactions so removal cannot race a reaction/source write.
    diesel::sql_query("SELECT c.id FROM conversations c JOIN messages m ON m.conversation_id=c.id WHERE m.id=$1 FOR SHARE OF c")
        .bind::<SqlUuid,_>(message).load::<Id>(conn)?;
    let allowed = diesel::sql_query("SELECT conversation_message_readable($1,$2) AS value")
        .bind::<SqlUuid, _>(message)
        .bind::<SqlUuid, _>(user)
        .get_result::<Flag>(conn)?
        .value;
    if allowed {
        Ok(())
    } else {
        Err(forbidden())
    }
}
fn member(user: Uuid, id: Uuid, conn: &mut PgConnection) -> Result<MembershipRow, PpdcError> {
    diesel::sql_query(
        "SELECT cm.* FROM conversation_members cm WHERE cm.conversation_id=$1 AND cm.user_id=$2
        AND cm.left_at IS NULL AND conversation_readable($1,$2)",
    )
    .bind::<SqlUuid, _>(id)
    .bind::<SqlUuid, _>(user)
    .get_result(conn)
    .optional()?
    .ok_or_else(forbidden)
}
fn lock(
    user: Uuid,
    id: Uuid,
    conn: &mut PgConnection,
) -> Result<(ConversationRow, MembershipRow), PpdcError> {
    let row = diesel::sql_query("SELECT * FROM conversations WHERE id=$1 FOR UPDATE")
        .bind::<SqlUuid, _>(id)
        .get_result(conn)
        .optional()?
        .ok_or_else(forbidden)?;
    let membership = member(user, id, conn)?;
    Ok((row, membership))
}
fn group_admin(c: &ConversationRow, m: &MembershipRow) -> Result<(), PpdcError> {
    if c.kind != "GROUP" || m.role != "ADMIN" {
        Err(forbidden())
    } else {
        Ok(())
    }
}
fn title(value: Option<String>) -> Result<Option<String>, PpdcError> {
    let value = value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if value.as_ref().is_some_and(|s| s.chars().count() > 200) {
        return Err(invalid("title must not exceed 200 characters"));
    }
    Ok(value)
}
fn validate_targets(actor: Uuid, ids: &[Uuid], pool: &DbPool) -> Result<(), PpdcError> {
    if ids.is_empty()
        || ids.len() > 49
        || ids.iter().copied().collect::<HashSet<_>>().len() != ids.len()
        || ids.contains(&actor)
    {
        return Err(invalid("Provide 1 to 49 distinct other users"));
    }
    for id in ids {
        crate::entities_v2::message::routes::ensure_can_send_message(actor, *id, pool)?;
    }
    Ok(())
}
fn add_locked(
    c: &ConversationRow,
    actor: Uuid,
    ids: &[Uuid],
    pool: &DbPool,
    conn: &mut PgConnection,
) -> Result<Vec<MemberAdded>, PpdcError> {
    let existing=diesel::sql_query("SELECT user_id AS id FROM conversation_members WHERE conversation_id=$1 AND left_at IS NULL")
        .bind::<SqlUuid,_>(c.id).load::<Id>(conn)?.into_iter().map(|r|r.id).collect::<HashSet<_>>();
    if existing.union(&ids.iter().copied().collect()).count() > 50 {
        return Err(invalid("Groups support at most 50 active members"));
    }
    let all = existing
        .union(&ids.iter().copied().collect())
        .copied()
        .collect::<Vec<_>>();
    for user in User::find_many(&all, pool)? {
        if user.principal_type != UserPrincipalType::Human
            || user.has_role(crate::entities_v2::user::UserRole::Mentor, pool)?
        {
            return Err(invalid("Mentors and service accounts cannot join groups"));
        }
    }
    for user in &all {
        crate::entities_v2::platform_infra::automation::ensure_same_sandbox(actor, *user, pool)?;
    }
    let joining = ids
        .iter()
        .copied()
        .filter(|id| !existing.contains(id))
        .collect::<Vec<_>>();
    if UserBlock::has_block_within_members(&all, &joining, pool)? {
        return Err(PpdcError::new(
            403,
            ErrorType::ApiError,
            "These members cannot be added to the same conversation".into(),
        ));
    }
    let mut added = Vec::new();
    for id in ids {
        let member = diesel::sql_query("INSERT INTO conversation_members(conversation_id,user_id,role,history_start_position,last_read_position)
            VALUES($1,$2,'MEMBER',$3,$4) ON CONFLICT(conversation_id,user_id) DO UPDATE
            SET role='MEMBER',joined_at=NOW(),left_at=NULL,history_start_position=$3,last_read_position=$4,muted_at=NULL
            WHERE conversation_members.left_at IS NOT NULL RETURNING *")
            .bind::<SqlUuid,_>(c.id).bind::<SqlUuid,_>(id).bind::<BigInt,_>(c.last_message_position+1)
            .bind::<BigInt,_>(c.last_message_position).get_result::<MembershipRow>(conn).optional()?;
        if let Some(member) = member {
            added.push(MemberAdded {
                conversation_id: c.id,
                recipient_user_id: member.user_id,
                actor_user_id: actor,
                joined_at: member.joined_at,
            });
        }
    }
    Ok(added)
}

pub(super) struct CreatedConversation {
    pub conversation: ConversationView,
    pub added_members: Vec<MemberAdded>,
}

pub(super) fn create(
    actor: Uuid,
    input: CreateConversation,
    pool: &DbPool,
) -> Result<CreatedConversation, PpdcError> {
    validate_targets(actor, &input.member_user_ids, pool)?;
    let name = title(input.title)?;
    let mut conn = pool.get()?;
    let (id, added_members)=conn.transaction::<_,PpdcError,_>(|conn| {
        if input.kind==ConversationKind::Direct {
            if input.member_user_ids.len()!=1 || name.is_some() {return Err(invalid("Direct conversations require exactly one other user and no title"));}
            let id = diesel::sql_query("SELECT ensure_direct_conversation($1,$2) AS id").bind::<SqlUuid,_>(actor)
                .bind::<SqlUuid,_>(input.member_user_ids[0]).get_result::<Id>(conn)?.id;
            return Ok((id, Vec::new()));
        }
        let row=diesel::sql_query("INSERT INTO conversations(kind,title,created_by_user_id) VALUES('GROUP',$1,$2) RETURNING *")
            .bind::<Nullable<Text>,_>(name).bind::<SqlUuid,_>(actor).get_result::<ConversationRow>(conn)?;
        diesel::sql_query("INSERT INTO conversation_members(conversation_id,user_id,role) VALUES($1,$2,'ADMIN')")
            .bind::<SqlUuid,_>(row.id).bind::<SqlUuid,_>(actor).execute(conn)?;
        let added = add_locked(&row,actor,&input.member_user_ids,pool,conn)?;
        Ok((row.id, added))
    })?;
    drop(conn);
    Ok(CreatedConversation {
        conversation: get(actor, id, pool)?,
        added_members,
    })
}

pub(super) fn add_members(
    actor: Uuid,
    id: Uuid,
    ids: Vec<Uuid>,
    pool: &DbPool,
) -> Result<Vec<MemberAdded>, PpdcError> {
    validate_targets(actor, &ids, pool)?;
    pool.get()?.transaction(|conn| {
        let (c, m) = lock(actor, id, conn)?;
        group_admin(&c, &m)?;
        add_locked(&c, actor, &ids, pool, conn)
    })
}
pub fn change_member(
    actor: Uuid,
    id: Uuid,
    target: Uuid,
    role: Option<MemberRole>,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    pool.get()?.transaction(|conn| {
        let(c,m)=lock(actor,id,conn)?;
        if c.kind!="GROUP" {return Err(invalid("Direct conversation membership is fixed; use mute instead"));}
        if actor!=target || role.is_some() {group_admin(&c,&m)?;}
        let target_member=diesel::sql_query("SELECT * FROM conversation_members WHERE conversation_id=$1 AND user_id=$2 AND left_at IS NULL")
            .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(target).get_result::<MembershipRow>(conn).optional()?;
        let Some(target_member)=target_member else {return Ok(());};
        if target_member.role=="ADMIN" && role!=Some(MemberRole::Admin) {
            let others=diesel::sql_query("SELECT COUNT(*)::bigint AS count FROM conversation_members
                WHERE conversation_id=$1 AND user_id<>$2 AND left_at IS NULL")
                .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(target).get_result::<Count>(conn)?.count;
            let admins=diesel::sql_query("SELECT COUNT(*)::bigint AS count FROM conversation_members
                WHERE conversation_id=$1 AND user_id<>$2 AND role='ADMIN' AND left_at IS NULL")
                .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(target).get_result::<Count>(conn)?.count;
            if others>0 && admins==0 {return Err(invalid("Promote another administrator before leaving or demoting the last admin"));}
        }
        if let Some(role)=role {
            diesel::sql_query("UPDATE conversation_members SET role=$3 WHERE conversation_id=$1 AND user_id=$2")
                .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(target).bind::<Text,_>(role.to_db()).execute(conn)?;
        } else {
            diesel::sql_query("UPDATE conversation_members SET left_at=NOW() WHERE conversation_id=$1 AND user_id=$2")
                .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(target).execute(conn)?;
        }
        Ok(())
    })
}
pub fn patch(
    actor: Uuid,
    id: Uuid,
    input: PatchConversation,
    pool: &DbPool,
) -> Result<ConversationView, PpdcError> {
    pool.get()?.transaction::<_,PpdcError,_>(|conn| {
        let(c,m)=lock(actor,id,conn)?;
        if let Some(name)=input.title {group_admin(&c,&m)?;
            diesel::sql_query("UPDATE conversations SET title=$2,updated_at=NOW() WHERE id=$1")
                .bind::<SqlUuid,_>(id).bind::<Nullable<Text>,_>(title(name)?).execute(conn)?;
        }
        if let Some(muted)=input.muted {
            diesel::sql_query("UPDATE conversation_members SET muted_at=CASE WHEN $3 THEN NOW() ELSE NULL END WHERE conversation_id=$1 AND user_id=$2")
                .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(actor).bind::<Bool,_>(muted).execute(conn)?;
        }
        Ok(())
    })?;
    get(actor, id, pool)
}

fn member_views(
    rows: Vec<MembershipRow>,
    pool: &DbPool,
) -> Result<Vec<(Uuid, MemberView)>, PpdcError> {
    let ids = rows.iter().map(|r| r.user_id).collect::<Vec<_>>();
    let users = User::find_many(&ids, pool)?;
    let asset_ids = users
        .iter()
        .filter_map(|u| u.profile_picture_asset_id)
        .collect::<Vec<_>>();
    let urls = Asset::find_public_urls_by_ids(&asset_ids, pool)?;
    let users = users
        .into_iter()
        .map(|u| (u.id, u))
        .collect::<HashMap<_, _>>();
    rows.into_iter()
        .map(|r| {
            let user = users.get(&r.user_id).ok_or_else(forbidden)?;
            let mut response = UserPublicResponse::from(user);
            response.profile_picture_display_url = user
                .profile_picture_asset_id
                .and_then(|id| urls.get(&id).cloned())
                .or(user.profile_picture_url.clone());
            Ok((
                r.conversation_id,
                MemberView {
                    user: response,
                    role: MemberRole::from_db(&r.role)?,
                    joined_at: r.joined_at,
                },
            ))
        })
        .collect()
}

#[derive(QueryableByName)]
struct SummaryRow {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    last_message_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<Timestamp>)]
    last_message_at: Option<chrono::NaiveDateTime>,
    #[diesel(sql_type=BigInt)]
    unread_count: i64,
    #[diesel(sql_type=BigInt)]
    members_count: i64,
}
fn views(
    user: Uuid,
    rows: Vec<ConversationRow>,
    pool: &DbPool,
) -> Result<Vec<ConversationView>, PpdcError> {
    if rows.is_empty() {
        return Ok(vec![]);
    }
    let ids = rows.iter().map(|r| r.id).collect::<Vec<_>>();
    let mut conn = pool.get()?;
    let summaries=diesel::sql_query("SELECT c.id,
        (SELECT m.id FROM messages m WHERE m.conversation_id=c.id AND m.processing_state='PROCESSED'
            AND conversation_message_readable(m.id,$2) ORDER BY m.position DESC LIMIT 1) AS last_message_id,
        (SELECT MAX(m.created_at) FROM messages m WHERE m.conversation_id=c.id AND m.processing_state='PROCESSED'
            AND conversation_message_readable(m.id,$2)) AS last_message_at,
        (SELECT COUNT(*) FROM messages m WHERE m.conversation_id=c.id AND conversation_message_unread(m.id,$2))::bigint AS unread_count,
        (SELECT COUNT(*) FROM conversation_members cm WHERE cm.conversation_id=c.id AND cm.left_at IS NULL)::bigint AS members_count
        FROM conversations c WHERE c.id=ANY($1) AND conversation_readable(c.id,$2)")
        .bind::<Array<SqlUuid>,_>(&ids).bind::<SqlUuid,_>(user).load::<SummaryRow>(&mut conn)?;
    let own=diesel::sql_query("SELECT * FROM conversation_members WHERE conversation_id=ANY($1) AND user_id=$2 AND left_at IS NULL")
        .bind::<Array<SqlUuid>,_>(&ids).bind::<SqlUuid,_>(user).load::<MembershipRow>(&mut conn)?;
    let preview=diesel::sql_query("SELECT conversation_id,user_id,role,joined_at,left_at,history_start_position,last_read_position,muted_at
        FROM (SELECT cm.*,ROW_NUMBER() OVER(PARTITION BY conversation_id ORDER BY joined_at,user_id) AS n
            FROM conversation_members cm WHERE conversation_id=ANY($1) AND left_at IS NULL) p WHERE n<=4")
        .bind::<Array<SqlUuid>,_>(&ids).load::<MembershipRow>(&mut conn)?;
    let message_ids = summaries
        .iter()
        .filter_map(|r| r.last_message_id)
        .collect::<Vec<_>>();
    let loaded = Message::find_many_with_conn(&message_ids, &mut conn)?;
    let mut messages = MessageReaction::hydrate_messages_with_conn(user, loaded, &mut conn)?
        .into_iter()
        .map(|m| (m.id, m))
        .collect::<HashMap<_, _>>();
    drop(conn);
    let mut preview_map: HashMap<Uuid, Vec<MemberView>> = HashMap::new();
    for (id, m) in member_views(preview, pool)? {
        preview_map.entry(id).or_default().push(m);
    }
    let mut summaries = summaries
        .into_iter()
        .map(|s| (s.id, s))
        .collect::<HashMap<_, _>>();
    let mut own = own
        .into_iter()
        .map(|m| (m.conversation_id, m))
        .collect::<HashMap<_, _>>();
    let mut result = Vec::new();
    for c in rows {
        let Some(s) = summaries.remove(&c.id) else {
            continue;
        };
        let membership: MembershipView = own.remove(&c.id).ok_or_else(forbidden)?.try_into()?;
        let sort_at = s.last_message_at.unwrap_or(membership.joined_at);
        result.push(ConversationView {
            id: c.id,
            kind: ConversationKind::from_db(&c.kind)?,
            title: c.title,
            created_by_user_id: c.created_by_user_id,
            created_at: c.created_at,
            updated_at: c.updated_at,
            sort_at,
            membership,
            members_count: s.members_count,
            members_preview: preview_map.remove(&c.id).unwrap_or_default(),
            last_message: s.last_message_id.and_then(|id| messages.remove(&id)),
            unread_count: s.unread_count,
        });
    }
    Ok(result)
}
pub fn get(user: Uuid, id: Uuid, pool: &DbPool) -> Result<ConversationView, PpdcError> {
    let mut conn = pool.get()?;
    member(user, id, &mut conn)?;
    let row = diesel::sql_query("SELECT * FROM conversations WHERE id=$1")
        .bind::<SqlUuid, _>(id)
        .get_result(&mut conn)?;
    drop(conn);
    views(user, vec![row], pool)?.pop().ok_or_else(forbidden)
}
fn normalized_search(query: Option<&str>) -> Result<Option<&str>, PpdcError> {
    let query = query.map(str::trim).filter(|value| !value.is_empty());
    if query.is_some_and(|value| value.chars().count() > 200) {
        return Err(invalid("q must not exceed 200 characters"));
    }
    Ok(query)
}

// Reuse exactly the same access/search predicate for the count and page query.
// Search the current public display identity, never hidden real names, the
// viewer's own name, former members, or message history outside their access.
const CONVERSATION_SEARCH_FROM: &str = "FROM conversations c
    JOIN conversation_members cm ON cm.conversation_id=c.id
    WHERE cm.user_id=$1 AND cm.left_at IS NULL AND conversation_readable(c.id,$1)
      AND ($2::text IS NULL
        OR strpos(lower(unaccent(COALESCE(c.title,''))),lower(unaccent($2)))>0
        OR EXISTS(SELECT 1 FROM conversation_members peer JOIN users u ON u.id=peer.user_id
            WHERE peer.conversation_id=c.id AND peer.left_at IS NULL AND peer.user_id<>$1
              AND (strpos(lower(unaccent(u.handle)),lower(unaccent($2)))>0
                OR strpos(lower(unaccent(CASE WHEN u.pseudonymized THEN u.pseudonym
                    ELSE u.first_name || ' ' || u.last_name END)),lower(unaccent($2)))>0)))";

pub fn list(
    user: Uuid,
    page: ValidatedPagination,
    query: Option<&str>,
    pool: &DbPool,
) -> Result<PaginatedResponse<ConversationView>, PpdcError> {
    let query = normalized_search(query)?;
    let mut conn = pool.get()?;
    let total = diesel::sql_query(format!(
        "SELECT COUNT(*)::bigint AS count {CONVERSATION_SEARCH_FROM}"
    ))
    .bind::<SqlUuid, _>(user)
    .bind::<Nullable<Text>, _>(query)
    .get_result::<Count>(&mut conn)?
    .count;
    let rows = diesel::sql_query(format!(
        "SELECT c.* {CONVERSATION_SEARCH_FROM}
        ORDER BY COALESCE((SELECT MAX(m.created_at) FROM messages m
            WHERE m.conversation_id=c.id AND m.processing_state='PROCESSED'
              AND m.position>=cm.history_start_position),cm.joined_at) DESC,
            c.created_at DESC,c.id
        LIMIT $3 OFFSET $4"
    ))
    .bind::<SqlUuid, _>(user)
    .bind::<Nullable<Text>, _>(query)
    .bind::<BigInt, _>(page.limit)
    .bind::<BigInt, _>(page.offset)
    .load(&mut conn)?;
    drop(conn);
    Ok(PaginatedResponse::new(
        views(user, rows, pool)?,
        page,
        total,
    ))
}
pub fn members(
    user: Uuid,
    id: Uuid,
    page: ValidatedPagination,
    pool: &DbPool,
) -> Result<PaginatedResponse<MemberView>, PpdcError> {
    let mut conn = pool.get()?;
    member(user, id, &mut conn)?;
    let total=diesel::sql_query("SELECT COUNT(*)::bigint AS count FROM conversation_members WHERE conversation_id=$1 AND left_at IS NULL")
        .bind::<SqlUuid,_>(id).get_result::<Count>(&mut conn)?.count;
    let rows=diesel::sql_query("SELECT * FROM conversation_members WHERE conversation_id=$1 AND left_at IS NULL ORDER BY joined_at,user_id LIMIT $2 OFFSET $3")
        .bind::<SqlUuid,_>(id).bind::<BigInt,_>(page.limit).bind::<BigInt,_>(page.offset).load(&mut conn)?;
    drop(conn);
    Ok(PaginatedResponse::new(
        member_views(rows, pool)?
            .into_iter()
            .map(|(_, m)| m)
            .collect(),
        page,
        total,
    ))
}
pub fn messages(
    user: Uuid,
    id: Uuid,
    page: ValidatedPagination,
    before: Option<i64>,
    pool: &DbPool,
) -> Result<PaginatedResponse<Message>, PpdcError> {
    if page.offset != 0 || before.is_some_and(|p| p <= 0) {
        return Err(invalid(
            "Use before_position with a positive position instead of offset",
        ));
    }
    let mut conn = pool.get()?;
    member(user, id, &mut conn)?;
    let total = diesel::sql_query(
        "SELECT COUNT(*)::bigint AS count FROM messages WHERE conversation_id=$1
        AND processing_state='PROCESSED' AND conversation_message_readable(id,$2)",
    )
    .bind::<SqlUuid, _>(id)
    .bind::<SqlUuid, _>(user)
    .get_result::<Count>(&mut conn)?
    .count;
    let ids=diesel::sql_query("SELECT id FROM messages WHERE conversation_id=$1 AND processing_state='PROCESSED'
        AND conversation_message_readable(id,$2) AND ($3::bigint IS NULL OR position<$3) ORDER BY position DESC LIMIT $4")
        .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(user).bind::<Nullable<BigInt>,_>(before).bind::<BigInt,_>(page.limit).load::<Id>(&mut conn)?;
    let mut map =
        Message::find_many_with_conn(&ids.iter().map(|r| r.id).collect::<Vec<_>>(), &mut conn)?
            .into_iter()
            .map(|m| (m.id, m))
            .collect::<HashMap<_, _>>();
    drop(conn);
    let messages = ids.into_iter().filter_map(|r| map.remove(&r.id)).collect();
    Ok(PaginatedResponse::new(
        MessageReaction::hydrate_messages(user, messages, pool)?,
        page,
        total,
    ))
}
pub fn read(
    user: Uuid,
    id: Uuid,
    through: i64,
    pool: &DbPool,
) -> Result<MembershipView, PpdcError> {
    let mut conn = pool.get()?;
    let m = conn.transaction::<_, PpdcError, _>(|conn| {
        let (c, m) = lock(user, id, conn)?;
        if through < m.history_start_position - 1 || through > c.last_message_position {
            return Err(invalid("Read position is outside the visible conversation"));
        }
        if c.kind == "DIRECT" {
            diesel::sql_query(
                "UPDATE messages SET seen_at=COALESCE(seen_at,NOW()) WHERE conversation_id=$1
                AND recipient_user_id=$2 AND position<=$3 AND processing_state='PROCESSED'",
            )
            .bind::<SqlUuid, _>(id)
            .bind::<SqlUuid, _>(user)
            .bind::<BigInt, _>(through)
            .execute(conn)?;
        }
        // Pending mentor placeholders must not become read before they have content.
        let cursor = if c.kind == "DIRECT" {
            diesel::sql_query(
                "SELECT COALESCE(MIN(position)-1,$3)::bigint AS count FROM messages
                WHERE conversation_id=$1 AND recipient_user_id=$2 AND seen_at IS NULL
                  AND processing_state IN ('PENDING','RUNNING') AND position<=$3",
            )
            .bind::<SqlUuid, _>(id)
            .bind::<SqlUuid, _>(user)
            .bind::<BigInt, _>(through)
            .get_result::<Count>(conn)?
            .count
        } else {
            through
        };
        Ok(diesel::sql_query(
            "UPDATE conversation_members SET last_read_position=GREATEST(last_read_position,$3)
            WHERE conversation_id=$1 AND user_id=$2 RETURNING *",
        )
        .bind::<SqlUuid, _>(id)
        .bind::<SqlUuid, _>(user)
        .bind::<BigInt, _>(cursor)
        .get_result::<MembershipRow>(conn)?)
    })?;
    m.try_into()
}

pub fn send(user: Uuid, id: Uuid, input: SendMessage, pool: &DbPool) -> Result<Message, PpdcError> {
    if input
        .message_type
        .is_some_and(|t| t != crate::entities_v2::message::MessageType::General)
        || input.attachment_type.is_some()
        || input.attachment.is_some()
        || input.writing_thread_id.is_some()
        || input.landscape_analysis_id.is_some()
        || input.expected_version_integer.is_some()
    {
        return Err(invalid("Groups currently support ordinary text messages and trace/post links, not mentor requests or attachments"));
    }
    if input.content.trim().is_empty() || input.content.chars().count() > 50_000 {
        return Err(invalid(
            "Message content must contain 1 to 50000 characters",
        ));
    }
    let c = {
        let mut conn = pool.get()?;
        let (c, _) = lock(user, id, &mut conn)?;
        c
    };
    if c.kind == "DIRECT" {
        let recipient = if c.direct_user_low_id == Some(user) {
            c.direct_user_high_id
        } else {
            c.direct_user_low_id
        }
        .ok_or_else(forbidden)?;
        // Direct/mentor requests continue through the established implementation.
        return Err(invalid(&format!("Send direct and mentor messages through POST /messages with recipient_user_id={recipient}")));
    }
    let linked_trace = input
        .trace_id
        .map(|trace| Trace::find_full_trace(trace, pool))
        .transpose()?;
    let linked_post = input
        .post_id
        .map(|post| Post::find_full(post, pool))
        .transpose()?;
    if linked_trace.is_some() && linked_post.is_some() {
        return Err(invalid("Provide trace_id or post_id, not both"));
    }
    let mut conn = pool.get()?;
    let message_id=conn.transaction::<_,PpdcError,_>(|conn| {
        let(c,_)=lock(user,id,conn)?;
        let active=diesel::sql_query("SELECT user_id AS id FROM conversation_members WHERE conversation_id=$1 AND left_at IS NULL")
            .bind::<SqlUuid,_>(c.id).load::<Id>(conn)?;
        for member in active {
            if let Some(trace)=&linked_trace {if !trace.user_can_read(member.id,pool)? {return Err(invalid("Every member must be able to read the linked trace"));}}
            if let Some(post)=&linked_post {if !PostGrant::user_can_read_post(post,member.id,pool)? {return Err(invalid("Every member must be able to read the linked post"));}}
        }
        if let Some(reply)=input.reply_to_message_id {
            ensure_message_readable_on_connection(reply,user,conn)?;
            let target=diesel::sql_query("SELECT conversation_id AS id FROM messages WHERE id=$1 AND processing_state='PROCESSED'")
                .bind::<SqlUuid,_>(reply).get_result::<Id>(conn)?;
            if target.id!=id {return Err(invalid("Replies must belong to the same conversation"));}
        }
        Ok(diesel::sql_query("INSERT INTO messages(sender_user_id,conversation_id,message_type,processing_state,title,content,reply_to_message_id,trace_id,post_id)
            VALUES($1,$2,'GENERAL','PROCESSED',$3,$4,$5,$6,$7) RETURNING id")
            .bind::<SqlUuid,_>(user).bind::<SqlUuid,_>(id).bind::<Text,_>(input.title.unwrap_or_default()).bind::<Text,_>(input.content)
            .bind::<Nullable<SqlUuid>,_>(input.reply_to_message_id).bind::<Nullable<SqlUuid>,_>(input.trace_id)
            .bind::<Nullable<SqlUuid>,_>(input.post_id).get_result::<Id>(conn)?.id)
    })?;
    drop(conn);
    Message::find(message_id, pool)
}

pub fn notification_recipients(message: &Message, pool: &DbPool) -> Result<Vec<Uuid>, PpdcError> {
    let rows=diesel::sql_query("SELECT cm.user_id AS id FROM conversation_members cm
        WHERE cm.conversation_id=$1 AND cm.left_at IS NULL AND cm.muted_at IS NULL AND cm.user_id<>$2
          AND conversation_message_readable($3,cm.user_id)
          AND NOT EXISTS(SELECT 1 FROM user_blocks b WHERE (b.blocker_user_id=$2 AND b.blocked_user_id=cm.user_id)
            OR (b.blocked_user_id=$2 AND b.blocker_user_id=cm.user_id))")
        .bind::<SqlUuid,_>(message.conversation_id).bind::<SqlUuid,_>(message.sender_user_id).bind::<SqlUuid,_>(message.id)
        .load::<Id>(&mut *pool.get()?)?;
    Ok(rows.into_iter().map(|r| r.id).collect())
}

pub fn spawn_group_notification(message: Message, pool: DbPool) {
    tokio::spawn(async move {
        let result=async {
            let recipients=notification_recipients(&message,&pool)?;
            let mut notification=crate::entities_v2::platform_infra::push::message_received_notification(&message,&pool).await?;
            notification.data.insert("conversation_kind".into(),"group".into());
            notification.body=message.content.chars().take(160).collect();
            notification.data.insert("message_content".into(),message.content.chars().take(240).collect());
            let row=diesel::sql_query("SELECT * FROM conversations WHERE id=$1")
                .bind::<SqlUuid,_>(message.conversation_id).get_result::<ConversationRow>(&mut *pool.get()?)?;
            if let Some(title)=row.title {notification.data.insert("conversation_title".into(),title.clone());notification.title=format!("{} · {}",notification.title,title);}
            for recipient in recipients {
                if !notification_recipients(&message,&pool)?.contains(&recipient){continue;}
                if let Err(err)=crate::entities_v2::platform_infra::push::send_to_user(recipient,notification.clone(),&pool).await {
                    tracing::warn!(target:"notification",message_id=%message.id,recipient_user_id=%recipient,error=%err.message,"group_message_recipient_push_failed");
                }
            }
            Ok::<_,PpdcError>(())
        }.await;
        if let Err(err) = result {
            tracing::warn!(target:"notification",conversation_id=%message.conversation_id,message_id=%message.id,error=%err.message,"group_message_push_failed");
        }
    });
}

pub fn reaction_notification_allowed(
    message: &Message,
    actor: Uuid,
    target: Uuid,
    pool: &DbPool,
) -> Result<bool, PpdcError> {
    let value=diesel::sql_query("SELECT EXISTS(SELECT 1 FROM conversation_members cm
        WHERE conversation_id=$1 AND user_id=$2 AND left_at IS NULL AND muted_at IS NULL
          AND conversation_message_readable($3,$2) AND conversation_message_readable($3,$4)
          AND NOT EXISTS(SELECT 1 FROM user_blocks b WHERE (b.blocker_user_id=$2 AND b.blocked_user_id=$4)
            OR (b.blocker_user_id=$4 AND b.blocked_user_id=$2))) AS value")
        .bind::<SqlUuid,_>(message.conversation_id).bind::<SqlUuid,_>(target).bind::<SqlUuid,_>(message.id)
        .bind::<SqlUuid,_>(actor).get_result::<Flag>(&mut *pool.get()?)?.value;
    Ok(value)
}
