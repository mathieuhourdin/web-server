use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

use super::{model::*, persist};
use crate::db::DbPool;
use crate::entities_v2::{
    error::PpdcError,
    session::Session,
    user::{User, UserPublicResponse},
};
use crate::pagination::{PaginatedResponse, PaginationParams};

fn actor(session: &Session) -> Result<Uuid, PpdcError> {
    session.user_id.ok_or_else(PpdcError::unauthorized)
}

pub(crate) fn notify(
    owner: Uuid,
    reader: Uuid,
    aid: Uuid,
    pending: bool,
    count: i64,
    pool: DbPool,
) {
    tokio::spawn(async move {
        let result = async {
            let audience = {
                let mut conn = pool.get()?;
                persist::owned(&mut conn, owner, aid)?
            };
            let subject = User::find(&reader, &pool)?;
            let publisher = User::find(&owner, &pool)?;
            let recipient = if pending { owner } else { reader };
            let event = if pending {
                "audience_history_review_pending"
            } else {
                "audience_history_shared"
            };
            let actor_user = if pending { &subject } else { &publisher };
            let mut data = HashMap::from([
                ("event_type".to_string(), event.to_string()),
                ("actor_user_id".to_string(), actor_user.id.to_string()),
                ("actor_display_name".to_string(), actor_user.display_name()),
            ]);
            if pending {
                data.insert("audience_id".to_string(), aid.to_string());
                data.insert("audience_name".to_string(), audience.name.clone());
                data.insert("member_user_id".to_string(), reader.to_string());
            } else {
                data.insert("shared_post_count".to_string(), count.to_string());
            }
            if let Some(url) = crate::entities_v2::push::sender_avatar_url(actor_user, &pool).await
            {
                data.insert("actor_avatar_url".to_string(), url);
            }
            let notification = crate::entities_v2::push::PushNotification {
                title: actor_user.display_name(),
                body: if pending {
                    format!(
                        "Choisissez l’historique à partager dans « {} »",
                        audience.name
                    )
                } else {
                    format!("vous a partagé {} traces", count)
                },
                data,
                thread_id: Some(actor_user.id.to_string()),
            };
            crate::entities_v2::push::send_to_mobile_user(recipient, notification, &pool).await
        }
        .await;
        match result {
            Ok(result) => {
                tracing::info!(target:"notification",audience_id=%aid,member_user_id=%reader,
                pending_review=pending,push_sent_count=result.sent_count,"audience_push_dispatch_completed")
            }
            Err(err) => {
                tracing::warn!(target:"notification",audience_id=%aid,member_user_id=%reader,
                pending_review=pending,error=%err.message,"audience_push_dispatch_failed")
            }
        }
    });
}

#[debug_handler]
pub async fn get_audiences(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(query): Query<AudienceListQuery>,
) -> Result<Json<PaginatedResponse<AudienceView>>, PpdcError> {
    let pagination = query.pagination.validate()?;
    let (items, total) = persist::list(
        actor(&session)?,
        query.include_archived,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    let items = persist::with_trace_counts(items, &pool)?;
    Ok(Json(PaginatedResponse::new(items, pagination, total)))
}

#[debug_handler]
pub async fn post_audience(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<AudienceNameDto>,
) -> Result<Json<Audience>, PpdcError> {
    Ok(Json(persist::create(
        actor(&session)?,
        &payload.name,
        &pool,
    )?))
}

#[debug_handler]
pub async fn get_audience(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(aid): Path<Uuid>,
) -> Result<Json<AudienceView>, PpdcError> {
    let audience = persist::owned(&mut *pool.get()?, actor(&session)?, aid)?;
    Ok(Json(
        persist::with_trace_counts(vec![audience], &pool)?.remove(0),
    ))
}

#[debug_handler]
pub async fn patch_audience(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(aid): Path<Uuid>,
    Json(payload): Json<AudienceNameDto>,
) -> Result<Json<Audience>, PpdcError> {
    Ok(Json(persist::rename(
        actor(&session)?,
        aid,
        &payload.name,
        &pool,
    )?))
}

#[debug_handler]
pub async fn delete_audience(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(aid): Path<Uuid>,
    Query(query): Query<RevokeHistoryQuery>,
) -> Result<Json<AudienceChange>, PpdcError> {
    Ok(Json(persist::archive(
        actor(&session)?,
        aid,
        query.revoke_history,
        &pool,
    )?))
}

#[debug_handler]
pub async fn get_members(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(aid): Path<Uuid>,
    Query(query): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<MemberView>>, PpdcError> {
    let pagination = query.validate()?;
    let (members, total) = persist::list_members(
        actor(&session)?,
        aid,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    let users = User::find_many(
        &members.iter().map(|m| m.user_id).collect::<Vec<_>>(),
        &pool,
    )?
    .into_iter()
    .map(|u| (u.id, u))
    .collect::<HashMap<_, _>>();
    let items = members
        .into_iter()
        .filter_map(|member| {
            users.get(&member.user_id).map(|u| MemberView {
                user: UserPublicResponse::from(u),
                member,
            })
        })
        .collect();
    Ok(Json(PaginatedResponse::new(items, pagination, total)))
}

#[debug_handler]
pub async fn put_member(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((aid, uid)): Path<(Uuid, Uuid)>,
    Json(payload): Json<AddMemberDto>,
) -> Result<Json<AudienceMember>, PpdcError> {
    let owner = actor(&session)?;
    let (member, pending) =
        persist::add_member(owner, aid, uid, payload.future_access_enabled, &pool)?;
    if pending {
        notify(owner, uid, aid, true, 0, pool);
    }
    Ok(Json(member))
}

#[debug_handler]
pub async fn get_member(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((aid, uid)): Path<(Uuid, Uuid)>,
) -> Result<Json<MemberView>, PpdcError> {
    let member = {
        let mut conn = pool.get()?;
        persist::owned(&mut conn, actor(&session)?, aid)?;
        persist::member(&mut conn, aid, uid)?
    };
    let user = User::find(&uid, &pool)?;
    Ok(Json(MemberView {
        member,
        user: UserPublicResponse::from(&user),
    }))
}

#[debug_handler]
pub async fn patch_member(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((aid, uid)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateMemberDto>,
) -> Result<Json<AudienceMember>, PpdcError> {
    Ok(Json(persist::change_future(
        actor(&session)?,
        aid,
        uid,
        payload.future_access_enabled,
        &pool,
    )?))
}

#[debug_handler]
pub async fn delete_member(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((aid, uid)): Path<(Uuid, Uuid)>,
    Query(query): Query<RemoveMemberQuery>,
) -> Result<Json<MemberChange>, PpdcError> {
    Ok(Json(persist::remove_member(
        actor(&session)?,
        aid,
        uid,
        query.stop_future,
        query.revoke_history,
        &pool,
    )?))
}

#[debug_handler]
pub async fn get_pending_reviews(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(query): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<PendingReview>>, PpdcError> {
    let owner = actor(&session)?;
    let pagination = query.validate()?;
    let (members, total) = persist::pending(owner, pagination.offset, pagination.limit, &pool)?;
    let users = User::find_many(
        &members.iter().map(|m| m.user_id).collect::<Vec<_>>(),
        &pool,
    )?
    .into_iter()
    .map(|u| (u.id, u))
    .collect::<HashMap<_, _>>();
    let mut items = Vec::new();
    for member in members {
        if let Some(user) = users.get(&member.user_id) {
            let audience = persist::owned(&mut *pool.get()?, owner, member.audience_id)?;
            let mut review_reasons = Vec::new();
            if member.status == "suggested" {
                review_reasons.push(AudienceReviewReason::FutureDefault);
            }
            if member.history_review_state != "reviewed" {
                review_reasons.push(AudienceReviewReason::History);
            }
            items.push(PendingReview {
                audience,
                user: UserPublicResponse::from(user),
                member,
                review_reasons,
            });
        }
    }
    Ok(Json(PaginatedResponse::new(items, pagination, total)))
}

#[debug_handler]
pub async fn get_history_summary(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((aid, uid)): Path<(Uuid, Uuid)>,
) -> Result<Json<HistorySummary>, PpdcError> {
    Ok(Json(persist::history_summary(
        actor(&session)?,
        aid,
        uid,
        &pool,
    )?))
}

#[debug_handler]
pub async fn get_history_posts(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((aid, uid)): Path<(Uuid, Uuid)>,
    Query(query): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<AudiencePost>>, PpdcError> {
    let pagination = query.validate()?;
    let (items, total) = persist::list_posts(
        actor(&session)?,
        aid,
        Some(uid),
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(items, pagination, total)))
}

#[debug_handler]
pub async fn post_history_decision(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((aid, uid)): Path<(Uuid, Uuid)>,
    Json(payload): Json<HistoryDecisionDto>,
) -> Result<Json<HistoryDecisionResult>, PpdcError> {
    let owner = actor(&session)?;
    let result = persist::apply_history(owner, aid, uid, payload, &pool)?;
    if result.newly_shared_post_count > 0 {
        notify(owner, uid, aid, false, result.newly_shared_post_count, pool);
    }
    Ok(Json(result))
}

#[debug_handler]
pub async fn post_history_review(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((aid, uid)): Path<(Uuid, Uuid)>,
) -> Result<Json<AudienceMember>, PpdcError> {
    Ok(Json(persist::restart_history_review(
        actor(&session)?,
        aid,
        uid,
        &pool,
    )?))
}

#[debug_handler]
pub async fn get_posts(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(aid): Path<Uuid>,
    Query(query): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<AudiencePost>>, PpdcError> {
    let pagination = query.validate()?;
    let (items, total) = persist::list_posts(
        actor(&session)?,
        aid,
        None,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(items, pagination, total)))
}

#[debug_handler]
pub async fn get_post_audiences(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(pid): Path<Uuid>,
) -> Result<Json<Vec<Audience>>, PpdcError> {
    Ok(Json(persist::post_audiences(actor(&session)?, pid, &pool)?))
}

#[debug_handler]
pub async fn put_post_audience(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((pid, aid)): Path<(Uuid, Uuid)>,
) -> Result<Json<AudienceChange>, PpdcError> {
    Ok(Json(persist::attach(actor(&session)?, aid, pid, &pool)?))
}

#[debug_handler]
pub async fn delete_post_audience(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((pid, aid)): Path<(Uuid, Uuid)>,
) -> Result<Json<AudienceChange>, PpdcError> {
    Ok(Json(persist::detach(
        actor(&session)?,
        aid,
        pid,
        true,
        &pool,
    )?))
}

#[derive(Deserialize)]
pub struct DefaultAudiencesDto {
    pub audience_ids: Vec<Uuid>,
}

#[debug_handler]
pub async fn get_journal_defaults(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(jid): Path<Uuid>,
) -> Result<Json<Vec<Audience>>, PpdcError> {
    Ok(Json(persist::default_audiences(
        actor(&session)?,
        jid,
        &pool,
    )?))
}

#[debug_handler]
pub async fn put_journal_defaults(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(jid): Path<Uuid>,
    Json(payload): Json<DefaultAudiencesDto>,
) -> Result<Json<Vec<Audience>>, PpdcError> {
    Ok(Json(persist::replace_defaults(
        actor(&session)?,
        jid,
        &payload.audience_ids,
        &pool,
    )?))
}
