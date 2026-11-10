//! Temporary conversation media, deliberately not a platform Asset.
use std::{
    collections::{HashMap, HashSet},
    io::Cursor,
    sync::OnceLock,
};

use axum::{
    extract::{Extension, Multipart, Path},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{NaiveDateTime, Utc};
use diesel::{
    prelude::*,
    sql_types::{Array, BigInt, Integer, Nullable, Text, Timestamp, Uuid as SqlUuid},
};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use uuid::Uuid;

use super::service;
use crate::{
    db::DbPool,
    entities_v2::{
        asset,
        error::{ErrorType, PpdcError},
        message::Message,
        session::Session,
    },
};

pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
const MAX_IMAGES: usize = 4;
const URL_TTL: i64 = 300;

fn error(status: u32, message: &str) -> PpdcError {
    PpdcError::new(
        status,
        if status >= 500 {
            ErrorType::InternalError
        } else {
            ErrorType::ApiError
        },
        message.to_string(),
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
struct StoredImage {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    message_id: Option<Uuid>,
    #[diesel(sql_type=Text)]
    bucket: String,
    #[diesel(sql_type=Text)]
    object_key: String,
    #[diesel(sql_type=Text)]
    mime_type: String,
    #[diesel(sql_type=BigInt)]
    size_bytes: i64,
    #[diesel(sql_type=Integer)]
    width: i32,
    #[diesel(sql_type=Integer)]
    height: i32,
    #[diesel(sql_type=Text)]
    status: String,
    #[diesel(sql_type=Timestamp)]
    created_at: NaiveDateTime,
    #[diesel(sql_type=Timestamp)]
    expires_at: NaiveDateTime,
}

/// No bucket/object key, uploader identity or signed bearer URL in message metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageImage {
    pub id: Uuid,
    pub mime_type: String,
    pub size_bytes: i64,
    pub width: i32,
    pub height: i32,
    pub created_at: NaiveDateTime,
    pub expires_at: NaiveDateTime,
    pub remote_available: bool,
}
impl StoredImage {
    fn view(&self) -> MessageImage {
        MessageImage {
            id: self.id,
            mime_type: self.mime_type.clone(),
            size_bytes: self.size_bytes,
            width: self.width,
            height: self.height,
            created_at: self.created_at,
            expires_at: self.expires_at,
            remote_available: self.status == "READY" && self.expires_at > Utc::now().naive_utc(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DownloadUrl {
    pub url: String,
    /// URL validity, NOT the remote image's retention deadline.
    pub expires_at: NaiveDateTime,
}

pub fn validate_ids(ids: &[Uuid]) -> Result<(), PpdcError> {
    if ids.len() > MAX_IMAGES || ids.iter().collect::<HashSet<_>>().len() != ids.len() {
        return Err(error(
            400,
            "Provide at most four distinct image_attachment_ids",
        ));
    }
    Ok(())
}

/// Same membership/block/history model as messages; direct media is human-only.
/// The conversation lock also serializes claims with leaving/rejoining.
fn ensure_conversation(
    user: Uuid,
    conversation: Uuid,
    conn: &mut PgConnection,
) -> Result<(), PpdcError> {
    let row=diesel::sql_query("SELECT c.id FROM conversations c
        WHERE c.id=$1 AND conversation_readable(c.id,$2)
        AND EXISTS(SELECT 1 FROM users WHERE id=$2 AND principal_type='HUMAN')
        AND (c.kind='GROUP' OR (
            EXISTS(SELECT 1 FROM users WHERE id=c.direct_user_low_id AND principal_type='HUMAN')
            AND EXISTS(SELECT 1 FROM users WHERE id=c.direct_user_high_id AND principal_type='HUMAN')
            AND NOT EXISTS(SELECT 1 FROM user_roles WHERE user_id IN(c.direct_user_low_id,c.direct_user_high_id) AND role='MENTOR')))
        FOR SHARE OF c")
        .bind::<SqlUuid,_>(conversation).bind::<SqlUuid,_>(user).get_result::<Id>(conn).optional()?;
    if !row.is_some_and(|row| row.id == conversation) {
        return Err(error(403, "Conversation image access denied"));
    }
    Ok(())
}

/// Called within the message-insertion transaction. A failed claim rolls back the
/// message and its conversation position, never leaving a half-sent message.
pub(crate) fn bind(message: Uuid, ids: &[Uuid], conn: &mut PgConnection) -> Result<(), PpdcError> {
    validate_ids(ids)?;
    if ids.is_empty() {
        return Ok(());
    }
    #[derive(QueryableByName)]
    struct Target {
        #[diesel(sql_type=SqlUuid)]
        conversation_id: Uuid,
        #[diesel(sql_type=SqlUuid)]
        sender_user_id: Uuid,
        #[diesel(sql_type=Text)]
        message_type: String,
    }
    let target = diesel::sql_query(
        "SELECT conversation_id,sender_user_id,message_type FROM messages WHERE id=$1",
    )
    .bind::<SqlUuid, _>(message)
    .get_result::<Target>(conn)?;
    if target.message_type != "GENERAL" {
        return Err(error(
            400,
            "Images are only supported on ordinary human messages",
        ));
    }
    ensure_conversation(target.sender_user_id, target.conversation_id, conn)?;
    let claimed=diesel::sql_query("UPDATE message_images i SET message_id=$1,position=array_position($2::uuid[],i.id)-1
        FROM conversation_members cm WHERE i.id=ANY($2) AND i.message_id IS NULL AND i.position IS NULL AND i.status='READY'
        AND i.expires_at>timezone('UTC',now()) AND i.conversation_id=$3 AND i.uploaded_by_user_id=$4
        AND cm.conversation_id=$3 AND cm.user_id=$4 AND cm.left_at IS NULL AND i.created_at>=cm.joined_at
        RETURNING i.id")
        .bind::<SqlUuid,_>(message).bind::<Array<SqlUuid>,_>(ids)
        .bind::<SqlUuid,_>(target.conversation_id).bind::<SqlUuid,_>(target.sender_user_id)
        .load::<Id>(conn)?;
    if claimed.len() != ids.len() {
        return Err(error(409,"An image is unavailable, already sent, or does not belong to this conversation and admission"));
    }
    Ok(())
}

pub(crate) fn hydrate(messages: &mut [Message], conn: &mut PgConnection) -> Result<(), PpdcError> {
    if messages.is_empty() {
        return Ok(());
    }
    let ids = messages.iter().map(|m| m.id).collect::<Vec<_>>();
    let rows=diesel::sql_query("SELECT * FROM message_images WHERE message_id=ANY($1) AND status='READY' ORDER BY message_id,position,id")
        .bind::<Array<SqlUuid>,_>(&ids).load::<StoredImage>(conn)?;
    let mut by_message = HashMap::<Uuid, Vec<MessageImage>>::new();
    for row in rows {
        by_message
            .entry(row.message_id.unwrap())
            .or_default()
            .push(row.view());
    }
    for message in messages {
        message.images = by_message.get(&message.id).cloned().unwrap_or_default();
    }
    Ok(())
}

fn validate_bytes(bytes: &[u8]) -> Result<(&'static str, i32, i32), PpdcError> {
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err(error(400, "Image must contain 1 to 5242880 bytes"));
    }
    let format = image::guess_format(bytes).map_err(|_| error(400, "Invalid image data"))?;
    let mime = match format {
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::WebP => "image/webp",
        _ => return Err(error(400, "Only JPEG, PNG and WebP images are supported")),
    };
    let (width, height) = image::io::Reader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|_| error(400, "Invalid image dimensions"))?;
    if width == 0
        || height == 0
        || width > 8192
        || height > 8192
        || u64::from(width) * u64::from(height) > 16_000_000
    {
        return Err(error(
            400,
            "Resize image to at most 16 million pixels and 8192 pixels per side",
        ));
    }
    let mut reader = image::io::Reader::with_format(Cursor::new(bytes), format);
    let mut limits = image::io::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(96 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|_| error(400, "Invalid or oversized image data"))?;
    Ok((mime, width as i32, height as i32))
}

fn reserve(
    user: Uuid,
    conversation: Uuid,
    bucket: &str,
    mime: &str,
    size: usize,
    width: i32,
    height: i32,
    pool: &DbPool,
) -> Result<StoredImage, PpdcError> {
    let mut conn = pool.get()?;
    conn.transaction::<_,PpdcError,_>(|conn| {
        ensure_conversation(user,conversation,conn)?;
        // One upload budget per user across conversations/VMs; failed uploads
        // count toward the daily budget too. No background rate-limit service.
        diesel::sql_query("SELECT pg_advisory_xact_lock(hashtextextended('conversation-image:'||$1::text,0))")
            .bind::<SqlUuid,_>(user).execute(conn)?;
        let count=diesel::sql_query("SELECT COUNT(*)::bigint AS count FROM message_images WHERE uploaded_by_user_id=$1
            AND created_at>timezone('UTC',now())-interval '24 hours'")
            .bind::<SqlUuid,_>(user).get_result::<Count>(conn)?.count;
        if count>=100 {return Err(error(429,"Conversation image upload limit reached"));}
        let id=Uuid::new_v4();
        diesel::sql_query("INSERT INTO message_images(id,conversation_id,uploaded_by_user_id,bucket,object_key,mime_type,size_bytes,width,height,status)
            VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,'UPLOADING') RETURNING *")
            .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(conversation).bind::<SqlUuid,_>(user)
            .bind::<Text,_>(bucket).bind::<Text,_>(format!("conversations/{conversation}/images/{id}"))
            .bind::<Text,_>(mime).bind::<BigInt,_>(size as i64).bind::<Integer,_>(width).bind::<Integer,_>(height)
            .get_result(conn).map_err(Into::into)
    })
}

pub async fn upload(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(conversation): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<MessageImage>), PpdcError> {
    let user = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let bucket = crate::environment::get_conversation_images_bucket()
        .ok_or_else(|| error(503, "Conversation image uploads are not configured"))?;
    if bucket == crate::environment::get_gcs_bucket_name()
        || Some(bucket.clone()) == crate::environment::get_gcs_public_assets_bucket_name()
    {
        return Err(error(
            503,
            "Conversation images require a dedicated temporary bucket",
        ));
    }
    let p = pool.clone();
    tokio::task::spawn_blocking(move || {
        let c = service::get(user, conversation, &p)?;
        if c.kind == super::model::ConversationKind::Direct {
            let other = c
                .members_preview
                .iter()
                .find(|m| m.user.id != user)
                .ok_or_else(|| error(403, "Conversation image access denied"))?;
            crate::entities_v2::message::routes::ensure_can_send_message(user, other.user.id, &p)?;
        }
        ensure_conversation(user, conversation, &mut *p.get()?)
    })
    .await
    .map_err(|_| error(500, "Image authorization failed"))??;
    static UPLOADS: OnceLock<Semaphore> = OnceLock::new();
    let _permit = UPLOADS
        .get_or_init(|| Semaphore::new(2))
        .try_acquire()
        .map_err(|_| error(429, "Too many image uploads in progress; try again shortly"))?;
    let mut bytes = None;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|_| error(400, "Invalid multipart upload"))?
    {
        if field.name() != Some("file") || bytes.is_some() {
            return Err(error(400, "Provide exactly one multipart field named file"));
        }
        let mut data = Vec::new();
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|_| error(400, "Invalid multipart image data"))?
        {
            if data.len() + chunk.len() > MAX_IMAGE_BYTES {
                return Err(error(413, "Image exceeds 5 MiB"));
            }
            data.extend_from_slice(&chunk);
        }
        bytes = Some(data);
    }
    let bytes = bytes.ok_or_else(|| error(400, "Provide a multipart file"))?;
    let p = pool.clone();
    let (bytes, mut image) = tokio::task::spawn_blocking(move || {
        let (mime, width, height) = validate_bytes(&bytes)?;
        let image = reserve(
            user,
            conversation,
            &bucket,
            mime,
            bytes.len(),
            width,
            height,
            &p,
        )?;
        Ok::<_, PpdcError>((bytes, image))
    })
    .await
    .map_err(|_| error(500, "Image validation failed"))??;
    let upload = asset::upload_conversation_image_to_gcs(
        &image.bucket,
        &image.object_key,
        &image.mime_type,
        bytes,
    )
    .await;
    let status = if upload.is_ok() { "READY" } else { "FAILED" };
    let p = pool.clone();
    let id = image.id;
    let saved=tokio::task::spawn_blocking(move || {
        let mut conn=p.get()?;
        conn.transaction::<_,PpdcError,_>(|conn| {
            ensure_conversation(user,conversation,conn)?;
            Ok(diesel::sql_query("UPDATE message_images SET status=$2 WHERE id=$1 AND status='UPLOADING'
                AND created_at>=(SELECT joined_at FROM conversation_members WHERE conversation_id=$3 AND user_id=$4 AND left_at IS NULL)")
                .bind::<SqlUuid,_>(id).bind::<Text,_>(status).bind::<SqlUuid,_>(conversation).bind::<SqlUuid,_>(user).execute(conn)?)
        })
    }).await.map_err(|_|error(500,"Image upload finalization failed"))?;
    if upload.is_err() || !matches!(saved, Ok(1)) {
        // Deletion is best-effort; a failed cleanup is still covered by GCS TTL.
        if let Err(err) = asset::delete_object_from_gcs(&image.bucket, &image.object_key).await {
            tracing::warn!(target:"conversation_images",image_id=%image.id,error=%err.message,"image_upload_cleanup_failed");
        }
        upload?;
        saved?;
        return Err(error(409, "Conversation membership changed during upload"));
    }
    image.status = "READY".into();
    Ok((StatusCode::CREATED, Json(image.view())))
}

pub async fn download_url(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((conversation, id)): Path<(Uuid, Uuid)>,
) -> Result<Response, PpdcError> {
    let user = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let image=tokio::task::spawn_blocking(move || {
        let mut conn=pool.get()?;
        let image=diesel::sql_query("SELECT * FROM message_images WHERE id=$1 AND conversation_id=$2 AND message_id IS NOT NULL")
            .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(conversation).get_result::<StoredImage>(&mut conn).optional()?
            .ok_or_else(||error(404,"Message image not found"))?;
        service::ensure_message_readable_on_connection(image.message_id.unwrap(),user,&mut conn)?;
        Ok::<_,PpdcError>(image)
    }).await.map_err(|_|error(500,"Image authorization failed"))??;
    let remaining = (image.expires_at - Utc::now().naive_utc()).num_seconds();
    if image.status != "READY" || remaining <= 1 {
        return Err(error(410, "Message image has expired"));
    }
    let (url, expires_at) = asset::sign_gcs_read_url(
        &image.bucket,
        &image.object_key,
        remaining.min(URL_TTL).saturating_sub(1) as u64,
    )
    .await?;
    if expires_at > image.expires_at {
        return Err(error(410, "Message image has expired"));
    }
    let mut response = Json(DownloadUrl { url, expires_at }).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(response)
}

/// Cancel only an unsent upload. Keep the row for quota accounting and purge.
/// Mark it FAILED before touching storage, so sending cannot race deletion.
pub async fn cancel(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((conversation, id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, PpdcError> {
    let user = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let image=tokio::task::spawn_blocking(move || {
        let mut conn=pool.get()?;
        conn.transaction::<_,PpdcError,_>(|conn| {
            ensure_conversation(user,conversation,conn)?;
            diesel::sql_query("UPDATE message_images SET status='FAILED'
                WHERE id=$1 AND conversation_id=$2 AND uploaded_by_user_id=$3 AND message_id IS NULL AND position IS NULL RETURNING *")
                .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(conversation).bind::<SqlUuid,_>(user)
                .get_result::<StoredImage>(conn).optional()?
                .ok_or_else(||error(409,"Image is already sent or is not your pending upload"))
        })
    }).await.map_err(|_|error(500,"Image cancellation failed"))??;
    asset::delete_object_from_gcs(&image.bucket, &image.object_key).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Called before the account's DB transaction, just like permanent Asset cleanup.
pub(crate) async fn purge_owned(user: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
    let rows = diesel::sql_query("SELECT * FROM message_images WHERE uploaded_by_user_id=$1")
        .bind::<SqlUuid, _>(user)
        .load::<StoredImage>(&mut *pool.get()?)?;
    for image in rows {
        asset::delete_object_from_gcs(&image.bucket, &image.object_key).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_images_and_unique_claims() {
        let id = Uuid::new_v4();
        assert!(validate_ids(&[id, id]).is_err());
        assert!(validate_ids(&[Uuid::new_v4(); 5]).is_err());
        assert!(validate_bytes(b"not an image").is_err());
        let mut png = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(3, 2)
            .write_to(&mut png, image::ImageOutputFormat::Png)
            .unwrap();
        assert_eq!(validate_bytes(png.get_ref()).unwrap(), ("image/png", 3, 2));
    }

    #[tokio::test]
    #[ignore = "Explicit dev-bucket smoke test: creates and deletes one generated image, no user data"]
    async fn temporary_image_gcs_round_trip() {
        let bucket = std::env::var("CONVERSATION_IMAGE_SMOKE_BUCKET").unwrap();
        assert_eq!(bucket, "hupo-conversation-images-dev");
        let key = format!("synthetic-smoke/{}", Uuid::new_v4());
        let mut png = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(3, 2)
            .write_to(&mut png, image::ImageOutputFormat::Png)
            .unwrap();
        let bytes = png.into_inner();
        let result = async {
            asset::upload_conversation_image_to_gcs(&bucket, &key, "image/png", bytes.clone())
                .await?;
            let (url, _) = asset::sign_gcs_read_url(&bucket, &key, 60).await?;
            let response = reqwest::get(url)
                .await
                .map_err(|_| error(500, "Synthetic signed download failed"))?;
            if !response.status().is_success() {
                return Err(error(500, "Synthetic signed download was denied"));
            }
            if response
                .headers()
                .get("cache-control")
                .and_then(|v| v.to_str().ok())
                != Some("private, no-store")
            {
                return Err(error(500, "Temporary image cache-control missing"));
            }
            let downloaded = response
                .bytes()
                .await
                .map_err(|_| error(500, "Synthetic download failed"))?;
            if downloaded.as_ref() != bytes.as_slice() {
                return Err(error(500, "Synthetic image content differs"));
            }
            Ok::<_, PpdcError>(())
        }
        .await;
        // Cleanup before assertions so even a failed assertion doesn't leave data.
        asset::delete_object_from_gcs(&bucket, &key).await.unwrap();
        asset::delete_object_from_gcs(&bucket, &key).await.unwrap();
        result.unwrap();
    }

    #[tokio::test]
    #[ignore = "Requires an explicitly configured disposable blog_test; synthetic rows only; no storage/LLM/push calls"]
    async fn image_claims_expiry_and_joining_history() {
        use super::super::model::{ConversationKind, CreateConversation};
        use crate::{
            entities_v2::message::{NewMessage, NewMessageDto},
            pagination::ValidatedPagination,
        };
        use diesel::{connection::SimpleConnection, r2d2::ConnectionManager};
        use diesel_migrations::{embed_migrations, MigrationHarness};
        let url = std::env::var("BLOG_TEST_DATABASE_URL").unwrap();
        assert_eq!(url::Url::parse(&url).unwrap().path(), "/blog_test");
        let pool = DbPool::builder()
            .max_size(8)
            .build(ConnectionManager::<PgConnection>::new(url))
            .unwrap();
        const MIGRATIONS: diesel_migrations::EmbeddedMigrations = embed_migrations!();
        pool.get()
            .unwrap()
            .run_pending_migrations(MIGRATIONS)
            .unwrap();
        let (a, b, c) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        struct Cleanup(DbPool, Vec<Uuid>);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                diesel::sql_query("DELETE FROM users WHERE id=ANY($1)")
                    .bind::<Array<SqlUuid>, _>(&self.1)
                    .execute(&mut *self.0.get().unwrap())
                    .unwrap();
            }
        }
        let _cleanup = Cleanup(pool.clone(), vec![a, b, c]);
        pool.get().unwrap().batch_execute(&format!("INSERT INTO users(id,email,first_name,last_name,handle,password)
            SELECT id,id::text||'@test.invalid','Synthetic','Image test',id::text,'unused'
            FROM unnest(ARRAY['{a}'::uuid,'{b}','{c}']) id;
            INSERT INTO relationships(requester_user_id,target_user_id,relationship_type,status,accepted_at)
            VALUES('{a}','{b}','FOLLOW','ACCEPTED',NOW()),('{a}','{c}','FOLLOW','ACCEPTED',NOW());")).unwrap();
        let make_group = || {
            service::create(
                a,
                CreateConversation {
                    kind: ConversationKind::Group,
                    title: None,
                    member_user_ids: vec![b],
                },
                &pool,
            )
            .unwrap()
            .conversation
            .id
        };
        let (group, other) = (make_group(), make_group());
        let seed = |conversation, user| {
            reserve(
                user,
                conversation,
                "synthetic-no-storage",
                "image/png",
                100,
                3,
                2,
                &pool,
            )
            .unwrap()
        };
        let ready = |image: StoredImage| {
            diesel::sql_query("UPDATE message_images SET status='READY' WHERE id=$1")
                .bind::<SqlUuid, _>(image.id)
                .execute(&mut *pool.get().unwrap())
                .unwrap();
            image.id
        };
        let own = ready(seed(group, a));
        let second = ready(seed(group, a));
        let foreign = ready(seed(group, b));
        let wrong_conversation = ready(seed(other, a));
        let send = |actor, ids: Vec<Uuid>| {
            service::send(
                actor,
                group,
                serde_json::from_value(
                    serde_json::json!({"content":"","image_attachment_ids":ids}),
                )
                .unwrap(),
                &pool,
            )
        };
        let message = send(a, vec![second, own]).unwrap();
        assert_eq!(
            message.images.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![second, own]
        );
        assert_eq!(message.notification_text(), "📷 2 photos");
        assert!(message.images[0].remote_available);
        let count = || {
            diesel::sql_query(
                "SELECT COUNT(*)::bigint AS count FROM messages WHERE conversation_id=$1",
            )
            .bind::<SqlUuid, _>(group)
            .get_result::<Count>(&mut *pool.get().unwrap())
            .unwrap()
            .count
        };
        for ids in [
            vec![own],
            vec![foreign],
            vec![wrong_conversation],
            vec![Uuid::new_v4()],
            vec![foreign, foreign],
        ] {
            assert!(send(a, ids).is_err());
            assert_eq!(count(), 1, "failed claims must roll back the message");
        }
        // A mixed valid/invalid claim must roll back the valid image too.
        let unclaimed = ready(seed(group, a));
        assert!(send(a, vec![unclaimed, foreign]).is_err());
        assert!(send(a, vec![unclaimed]).is_ok());
        service::add_members(a, group, vec![c], &pool).unwrap();
        let mut session = Session::anonymous();
        session.user_id = Some(c);
        assert_eq!(
            download_url(
                Extension(pool.clone()),
                Extension(session),
                Path((group, own))
            )
            .await
            .unwrap_err()
            .status_code,
            403
        );
        pool.get().unwrap().batch_execute(&format!("UPDATE message_images SET created_at=NOW()-interval '46 days',expires_at=NOW()-interval '1 day' WHERE id IN('{own}','{second}');")).unwrap();
        let mut session = Session::anonymous();
        session.user_id = Some(b);
        assert_eq!(
            download_url(
                Extension(pool.clone()),
                Extension(session),
                Path((group, own))
            )
            .await
            .unwrap_err()
            .status_code,
            410
        );
        let page = service::messages(
            b,
            group,
            ValidatedPagination {
                limit: 20,
                offset: 0,
            },
            None,
            &pool,
        )
        .unwrap();
        let image = &page
            .items
            .iter()
            .find(|m| m.id == message.id)
            .unwrap()
            .images[0];
        assert!(!image.remote_available);
        service::change_member(a, group, b, None, &pool).unwrap();
        service::add_members(a, group, vec![b], &pool).unwrap();
        assert!(service::ensure_message_readable(message.id, b, &pool).is_err());
        assert!(
            send(b, vec![foreign]).is_err(),
            "pending uploads do not survive rejoining"
        );
        // Legacy direct insertion follows the same transactional claim path.
        let direct = service::create(
            a,
            CreateConversation {
                kind: ConversationKind::Direct,
                title: None,
                member_user_ids: vec![b],
            },
            &pool,
        )
        .unwrap()
        .conversation
        .id;
        let image = ready(seed(direct, a));
        let input: NewMessageDto =
            serde_json::from_value(serde_json::json!({"recipient_user_id":b,"content":""}))
                .unwrap();
        let direct_message = NewMessage::new(input, a)
            .create_with_images(&[image], &pool)
            .unwrap();
        assert_eq!(direct_message.images.len(), 1);
        assert_eq!(direct_message.images[0].id, image);
        let wire = serde_json::to_value(&direct_message).unwrap();
        assert!(wire["images"][0].get("bucket").is_none());
        assert!(wire["images"][0].get("object_key").is_none());
    }
}
