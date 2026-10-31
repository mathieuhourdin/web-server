//! Publication events are distinct from live content and social access grants.
use super::{
    notifications::{NotificationOptions, PublicationNotificationMode},
    Post,
};
use crate::{
    db::DbPool,
    entities_v2::{
        error::{ErrorType, PpdcError},
        session::Session,
    },
    pagination::{PaginatedResponse, PaginationParams},
};
use axum::{
    extract::{Extension, Path, Query},
    Json,
};
use chrono::NaiveDateTime;
use diesel::{
    prelude::*,
    sql_types::{Array, BigInt, Bool, Integer, Nullable, Text, Timestamp, Uuid as SqlUuid},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn error(code: u32, text: &str) -> PpdcError {
    PpdcError::new(code, ErrorType::ApiError, text.into())
}
#[derive(Clone, Serialize, QueryableByName)]
pub struct Publication {
    #[diesel(sql_type=SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type=SqlUuid)]
    pub post_id: Uuid,
    #[diesel(sql_type=Timestamp)]
    pub published_at: NaiveDateTime,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    pub trace_version_id: Option<Uuid>,
    #[diesel(sql_type=Text)]
    pub notification_mode: String,
    #[diesel(sql_type=Array<SqlUuid>)]
    pub notification_user_ids: Vec<Uuid>,
    #[diesel(sql_type=SqlUuid)]
    pub idempotency_key: Uuid,
    #[diesel(sql_type=Nullable<Timestamp>)]
    pub dispatch_started_at: Option<NaiveDateTime>,
    #[diesel(sql_type=Timestamp)]
    pub created_at: NaiveDateTime,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationInput {
    pub idempotency_key: Uuid,
    pub trace_version_id: Option<Uuid>,
    #[serde(flatten)]
    pub notifications: NotificationOptions,
}
#[derive(Serialize, QueryableByName)]
pub struct Recipient {
    #[diesel(sql_type=SqlUuid)]
    pub user_id: Uuid,
    #[diesel(sql_type=Text)]
    pub push_status: String,
    #[diesel(sql_type=Nullable<Timestamp>)]
    pub push_sent_at: Option<NaiveDateTime>,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    pub email_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<Text>)]
    pub email_status: Option<String>,
    #[diesel(sql_type=Nullable<Timestamp>)]
    pub email_sent_at: Option<NaiveDateTime>,
}
#[derive(Serialize, QueryableByName)]
pub struct TraceVersion {
    #[diesel(sql_type=SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type=SqlUuid)]
    pub trace_id: Uuid,
    #[diesel(sql_type=Integer)]
    pub source_version: i32,
    #[diesel(sql_type=Text)]
    pub title: String,
    #[diesel(sql_type=Text)]
    pub subtitle: String,
    #[diesel(sql_type=Text)]
    pub content: String,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    pub content_image_asset_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<Text>)]
    pub language_code: Option<String>,
    #[diesel(sql_type=Nullable<Timestamp>)]
    pub finalized_at: Option<NaiveDateTime>,
    #[diesel(sql_type=Timestamp)]
    pub created_at: NaiveDateTime,
}
#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type=BigInt)]
    count: i64,
}
#[derive(QueryableByName)]
struct Id {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
}
#[derive(QueryableByName)]
struct Destination {
    #[diesel(sql_type=Bool)]
    channel: bool,
}

pub fn find(id: Uuid, pool: &DbPool) -> Result<Publication, PpdcError> {
    Ok(
        diesel::sql_query("SELECT * FROM post_publications WHERE id=$1")
            .bind::<SqlUuid, _>(id)
            .get_result(&mut pool.get()?)?,
    )
}
fn require_owner(post: Uuid, owner: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
    diesel::sql_query("SELECT id FROM posts WHERE id=$1 AND user_id=$2")
        .bind::<SqlUuid, _>(post)
        .bind::<SqlUuid, _>(owner)
        .get_result::<Id>(&mut pool.get()?)?;
    Ok(())
}
pub fn create(
    post: Uuid,
    owner: Uuid,
    input: PublicationInput,
    pool: &DbPool,
) -> Result<(Publication, bool), PpdcError> {
    pool.get()?.transaction::<_,PpdcError,_>(|conn|{
        let destination=diesel::sql_query("SELECT publication_channel_id IS NOT NULL AS channel FROM posts WHERE id=$1 AND user_id=$2 FOR UPDATE")
            .bind::<SqlUuid,_>(post).bind::<SqlUuid,_>(owner).get_result::<Destination>(conn)?;
        let mode=input.notifications.notification_mode.unwrap_or(if destination.channel{PublicationNotificationMode::None}else{PublicationNotificationMode::All});
        let mut ids=input.notifications.notification_user_ids.clone().unwrap_or_default();ids.sort_unstable();ids.dedup();
        if (mode==PublicationNotificationMode::Selected && (ids.is_empty() || ids.len()>500))
            || (mode!=PublicationNotificationMode::Selected && !ids.is_empty()) || (destination.channel && mode!=PublicationNotificationMode::None){
            return Err(error(400,"Invalid publication notification selection; blog/channel publications must use none"));
        }
        let existing=diesel::sql_query("SELECT * FROM post_publications WHERE post_id=$1 AND idempotency_key=$2")
            .bind::<SqlUuid,_>(post).bind::<SqlUuid,_>(input.idempotency_key).get_result::<Publication>(conn).optional()?;
        if let Some(existing)=existing {
            if existing.trace_version_id!=input.trace_version_id || existing.notification_mode!=mode.to_db() || existing.notification_user_ids!=ids {
                return Err(error(409,"This idempotency key already identifies a different publication request"));
            }
            return Ok((existing,false));
        }
        let row=diesel::sql_query("INSERT INTO post_publications(post_id,idempotency_key,trace_version_id,notification_mode,notification_user_ids)
            VALUES($1,$2,$3,$4,$5) RETURNING *")
            .bind::<SqlUuid,_>(post).bind::<SqlUuid,_>(input.idempotency_key).bind::<Nullable<SqlUuid>,_>(input.trace_version_id)
            .bind::<Text,_>(mode.to_db()).bind::<Array<SqlUuid>,_>(&ids).get_result(conn)?;
        Ok((row,true))
    })
}

/// Once-only dispatch claim, independent for each event (never keyed by post).
pub fn claim(id: Uuid, pool: &DbPool) -> Result<bool, PpdcError> {
    Ok(diesel::sql_query(
        "UPDATE post_publications e SET dispatch_started_at=NOW() FROM posts p
        WHERE e.id=$1 AND e.dispatch_started_at IS NULL AND p.id=e.post_id AND p.status='PUBLISHED'
            AND p.publication_channel_id IS NULL",
    )
    .bind::<SqlUuid, _>(id)
    .execute(&mut pool.get()?)?
        == 1)
}
pub fn eligible_recipients(post: &Post, pool: &DbPool) -> Result<Vec<Uuid>, PpdcError> {
    let Some(publication) = post.last_publication_id else {
        return Ok(vec![]);
    };
    Ok(diesel::sql_query("SELECT r.user_id AS id FROM post_publication_recipients r JOIN post_publications e ON e.id=r.publication_id
        WHERE r.publication_id=$1 AND e.post_id=$2 AND trace_publication_readable(e.post_id,r.user_id)")
        .bind::<SqlUuid,_>(publication).bind::<SqlUuid,_>(post.id).load::<Id>(&mut pool.get()?)?.into_iter().map(|r|r.id).collect())
}
pub fn targeted_posts(
    user: Uuid,
    posts: &[Uuid],
    pool: &DbPool,
) -> Result<std::collections::HashSet<Uuid>, PpdcError> {
    Ok(diesel::sql_query("SELECT p.id FROM social_posts p JOIN post_publication_recipients r ON r.publication_id=p.last_publication_id
        WHERE p.id=ANY($1) AND r.user_id=$2")
        .bind::<Array<SqlUuid>,_>(posts).bind::<SqlUuid,_>(user).load::<Id>(&mut pool.get()?)?.into_iter().map(|r|r.id).collect())
}
pub fn claim_push(publication: Uuid, user: Uuid, pool: &DbPool) -> Result<bool, PpdcError> {
    Ok(diesel::sql_query(
        "UPDATE post_publication_recipients r SET push_status='SENDING' FROM post_publications e
        WHERE r.publication_id=$1 AND r.user_id=$2 AND r.push_status='NOT_ATTEMPTED'
        AND e.id=r.publication_id AND trace_publication_readable(e.post_id,r.user_id)",
    )
    .bind::<SqlUuid, _>(publication)
    .bind::<SqlUuid, _>(user)
    .execute(&mut pool.get()?)?
        == 1)
}
pub fn finish_push(
    publication: Uuid,
    user: Uuid,
    sent: bool,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    diesel::sql_query("UPDATE post_publication_recipients SET push_status=$3,push_sent_at=CASE WHEN $4 THEN NOW() ELSE NULL END
        WHERE publication_id=$1 AND user_id=$2 AND push_status='SENDING'")
        .bind::<SqlUuid,_>(publication).bind::<SqlUuid,_>(user).bind::<Text,_>(if sent{"SENT"}else{"FAILED"}).bind::<Bool,_>(sent).execute(&mut pool.get()?)?;
    Ok(())
}
pub fn record_email(post: &Post, user: Uuid, email: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
    diesel::sql_query("UPDATE post_publication_recipients SET email_id=$3 WHERE publication_id=$1 AND user_id=$2 AND email_id IS NULL")
        .bind::<Nullable<SqlUuid>,_>(post.last_publication_id).bind::<SqlUuid,_>(user).bind::<SqlUuid,_>(email).execute(&mut pool.get()?)?;
    Ok(())
}
pub fn dispatch(event: &Publication, pool: &DbPool) -> Result<(), PpdcError> {
    if !claim(event.id, pool)? {
        return Ok(());
    }
    let mut post = Post::find_full(event.post_id, pool)?;
    post.last_publication_id = Some(event.id);
    post.last_publication_at = Some(event.published_at);
    post.notification_mode = PublicationNotificationMode::from_db(&event.notification_mode);
    post.notification_user_ids = event.notification_user_ids.clone();
    super::routes::dispatch_claimed_publication_notifications(&post, pool);
    Ok(())
}

pub async fn post_publication(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(post): Path<Uuid>,
    Json(input): Json<PublicationInput>,
) -> Result<Json<Publication>, PpdcError> {
    let owner = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let (event, created) = create(post, owner, input, &pool)?;
    if created {
        dispatch(&event, &pool)?;
    }
    Ok(Json(event))
}
pub async fn get_publications(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(post): Path<Uuid>,
    Query(page): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Publication>>, PpdcError> {
    require_owner(
        post,
        session.user_id.ok_or_else(PpdcError::unauthorized)?,
        &pool,
    )?;
    let page = page.validate()?;
    let mut conn = pool.get()?;
    let total = diesel::sql_query("SELECT COUNT(*) FROM post_publications WHERE post_id=$1")
        .bind::<SqlUuid, _>(post)
        .get_result::<Count>(&mut conn)?
        .count;
    let items=diesel::sql_query("SELECT * FROM post_publications WHERE post_id=$1 ORDER BY published_at DESC,id DESC LIMIT $2 OFFSET $3")
        .bind::<SqlUuid,_>(post).bind::<BigInt,_>(page.limit).bind::<BigInt,_>(page.offset).load(&mut conn)?;
    Ok(Json(PaginatedResponse::new(items, page, total)))
}
pub async fn get_recipients(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((post, event)): Path<(Uuid, Uuid)>,
    Query(page): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Recipient>>, PpdcError> {
    require_owner(
        post,
        session.user_id.ok_or_else(PpdcError::unauthorized)?,
        &pool,
    )?;
    if find(event, &pool)?.post_id != post {
        return Err(error(404, "Publication not found"));
    }
    let page = page.validate()?;
    let mut conn = pool.get()?;
    let total = diesel::sql_query(
        "SELECT COUNT(*) FROM post_publication_recipients WHERE publication_id=$1",
    )
    .bind::<SqlUuid, _>(event)
    .get_result::<Count>(&mut conn)?
    .count;
    let items=diesel::sql_query("SELECT r.user_id,r.push_status,r.push_sent_at,r.email_id,o.status AS email_status,o.sent_at AS email_sent_at
        FROM post_publication_recipients r LEFT JOIN outbound_emails o ON o.id=r.email_id
        WHERE r.publication_id=$1 ORDER BY r.user_id LIMIT $2 OFFSET $3")
        .bind::<SqlUuid,_>(event).bind::<BigInt,_>(page.limit).bind::<BigInt,_>(page.offset).load(&mut conn)?;
    Ok(Json(PaginatedResponse::new(items, page, total)))
}
fn require_trace_owner(trace: Uuid, owner: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
    diesel::sql_query("SELECT id FROM traces WHERE id=$1 AND user_id=$2")
        .bind::<SqlUuid, _>(trace)
        .bind::<SqlUuid, _>(owner)
        .get_result::<Id>(&mut pool.get()?)?;
    Ok(())
}
pub async fn archive_trace(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(trace): Path<Uuid>,
) -> Result<Json<TraceVersion>, PpdcError> {
    let owner = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    require_trace_owner(trace, owner, &pool)?;
    let row=diesel::sql_query("INSERT INTO trace_versions(trace_id,source_version,title,subtitle,content,content_image_asset_id,language_code,finalized_at)
        SELECT id,version_integer,title,subtitle,content,content_image_asset_id,language_code,finalized_at FROM traces
        WHERE id=$1 AND user_id=$2 AND status='FINALIZED' AND NOT is_encrypted RETURNING *")
        .bind::<SqlUuid,_>(trace).bind::<SqlUuid,_>(owner).get_result(&mut pool.get()?)?;
    Ok(Json(row))
}
pub async fn get_trace_versions(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(trace): Path<Uuid>,
    Query(page): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<TraceVersion>>, PpdcError> {
    require_trace_owner(
        trace,
        session.user_id.ok_or_else(PpdcError::unauthorized)?,
        &pool,
    )?;
    let page = page.validate()?;
    let mut conn = pool.get()?;
    let total = diesel::sql_query("SELECT COUNT(*) FROM trace_versions WHERE trace_id=$1")
        .bind::<SqlUuid, _>(trace)
        .get_result::<Count>(&mut conn)?
        .count;
    let items=diesel::sql_query("SELECT * FROM trace_versions WHERE trace_id=$1 ORDER BY created_at DESC,id DESC LIMIT $2 OFFSET $3")
        .bind::<SqlUuid,_>(trace).bind::<BigInt,_>(page.limit).bind::<BigInt,_>(page.offset).load(&mut conn)?;
    Ok(Json(PaginatedResponse::new(items, page, total)))
}
pub async fn get_trace_version(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((trace, version)): Path<(Uuid, Uuid)>,
) -> Result<Json<TraceVersion>, PpdcError> {
    require_trace_owner(
        trace,
        session.user_id.ok_or_else(PpdcError::unauthorized)?,
        &pool,
    )?;
    Ok(Json(
        diesel::sql_query("SELECT * FROM trace_versions WHERE trace_id=$1 AND id=$2")
            .bind::<SqlUuid, _>(trace)
            .bind::<SqlUuid, _>(version)
            .get_result(&mut pool.get()?)?,
    ))
}

#[derive(QueryableByName)]
pub struct PublicationMeta {
    #[diesel(sql_type=SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    pub last_publication_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<Timestamp>)]
    pub last_publication_at: Option<NaiveDateTime>,
    #[diesel(sql_type=Text)]
    pub notification_mode: String,
    #[diesel(sql_type=Array<SqlUuid>)]
    pub notification_user_ids: Vec<Uuid>,
}
impl PublicationMeta {
    pub fn apply(&self, post: &mut Post) {
        post.last_publication_id = self.last_publication_id;
        post.last_publication_at = self.last_publication_at;
        if post.status == super::PostStatus::Published {
            post.notification_mode = PublicationNotificationMode::from_db(&self.notification_mode);
            post.notification_user_ids = self.notification_user_ids.clone();
        }
    }
}
pub fn metadata(
    ids: &[Uuid],
    conn: &mut PgConnection,
) -> Result<std::collections::HashMap<Uuid, PublicationMeta>, PpdcError> {
    Ok(diesel::sql_query(
        "SELECT p.id,p.last_publication_id,p.last_publication_at,COALESCE(e.notification_mode,p.notification_mode) AS notification_mode,
        COALESCE(e.notification_user_ids,p.notification_user_ids) AS notification_user_ids
        FROM posts p LEFT JOIN post_publications e ON e.id=p.last_publication_id WHERE p.id=ANY($1)",
    )
    .bind::<Array<SqlUuid>, _>(ids)
    .load::<PublicationMeta>(conn)?
    .into_iter()
    .map(|r| (r.id, r))
    .collect())
}

#[cfg(test)]
mod tests;
