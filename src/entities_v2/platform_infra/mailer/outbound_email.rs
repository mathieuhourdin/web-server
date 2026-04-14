use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::Uuid as SqlUuid;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::schema::outbound_emails;

use super::{send_email, SendEmailRequest};

const EMAIL_RUN_LOCK_TTL_SECONDS: i64 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutboundEmailStatus {
    Pending,
    Running,
    Sent,
    Failed,
}

impl OutboundEmailStatus {
    pub fn to_db(self) -> &'static str {
        match self {
            OutboundEmailStatus::Pending => "PENDING",
            OutboundEmailStatus::Running => "RUNNING",
            OutboundEmailStatus::Sent => "SENT",
            OutboundEmailStatus::Failed => "FAILED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "RUNNING" => OutboundEmailStatus::Running,
            "SENT" => OutboundEmailStatus::Sent,
            "FAILED" => OutboundEmailStatus::Failed,
            _ => OutboundEmailStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutboundEmailProvider {
    Resend,
}

impl OutboundEmailProvider {
    pub fn to_db(self) -> &'static str {
        match self {
            OutboundEmailProvider::Resend => "RESEND",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "RESEND" => OutboundEmailProvider::Resend,
            _ => OutboundEmailProvider::Resend,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, Clone)]
#[diesel(table_name = crate::schema::outbound_emails)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct OutboundEmail {
    pub id: Uuid,
    pub recipient_user_id: Option<Uuid>,
    pub reason: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub to_email: String,
    pub from_email: String,
    pub subject: String,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    #[diesel(deserialize_as = String)]
    pub status: String,
    #[diesel(deserialize_as = String)]
    pub provider: String,
    pub provider_message_id: Option<String>,
    pub attempt_count: i32,
    pub last_error: Option<String>,
    pub lock_owner: Option<Uuid>,
    pub lock_until: Option<NaiveDateTime>,
    pub scheduled_at: Option<NaiveDateTime>,
    pub sent_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl OutboundEmail {
    pub fn status_enum(&self) -> OutboundEmailStatus {
        OutboundEmailStatus::from_db(&self.status)
    }

    pub fn provider_enum(&self) -> OutboundEmailProvider {
        OutboundEmailProvider::from_db(&self.provider)
    }

    pub fn find(id: Uuid, pool: &DbPool) -> Result<Self, PpdcError> {
        let mut conn = pool
            .get()?;
        let email = outbound_emails::table
            .select(Self::as_select())
            .filter(outbound_emails::id.eq(id))
            .first::<Self>(&mut conn)?;
        Ok(email)
    }

    pub fn list_due_pending(limit: i64, pool: &DbPool) -> Result<Vec<Self>, PpdcError> {
        let mut conn = pool
            .get()?;
        let emails = outbound_emails::table
            .filter(outbound_emails::status.eq(OutboundEmailStatus::Pending.to_db()))
            .filter(
                outbound_emails::scheduled_at
                    .is_null()
                    .or(outbound_emails::scheduled_at.le(diesel::dsl::now)),
            )
            .select(Self::as_select())
            .order((
                outbound_emails::scheduled_at.asc().nulls_first(),
                outbound_emails::created_at.asc(),
            ))
            .limit(limit)
            .load::<Self>(&mut conn)?;
        Ok(emails)
    }

    pub fn claim_due_pending(
        limit: i64,
        worker_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Self>, PpdcError> {
        let mut conn = pool
            .get()?;

        #[derive(diesel::QueryableByName)]
        struct IdRow {
            #[diesel(sql_type = SqlUuid)]
            id: Uuid,
        }

        let claimed_ids = sql_query(
            r#"
WITH candidate AS (
    SELECT oe.id
    FROM outbound_emails oe
    WHERE (
        (oe.status = 'PENDING' AND (oe.scheduled_at IS NULL OR oe.scheduled_at <= NOW()))
        OR (oe.status = 'RUNNING' AND oe.lock_until IS NOT NULL AND oe.lock_until <= NOW())
    )
    ORDER BY oe.scheduled_at ASC NULLS FIRST, oe.created_at ASC
    LIMIT $1
    FOR UPDATE SKIP LOCKED
)
UPDATE outbound_emails oe
SET status = 'RUNNING',
    lock_owner = $2,
    lock_until = NOW() + ($3 * INTERVAL '1 second'),
    updated_at = NOW()
FROM candidate
WHERE oe.id = candidate.id
RETURNING oe.id
            "#,
        )
        .bind::<diesel::sql_types::BigInt, _>(limit.max(1))
        .bind::<SqlUuid, _>(worker_id)
        .bind::<diesel::sql_types::BigInt, _>(EMAIL_RUN_LOCK_TTL_SECONDS)
        .get_results::<IdRow>(&mut conn)?;

        claimed_ids
            .into_iter()
            .map(|row| OutboundEmail::find(row.id, pool))
            .collect()
    }

    pub fn claim_if_due(id: Uuid, worker_id: Uuid, pool: &DbPool) -> Result<Option<Self>, PpdcError> {
        let mut conn = pool
            .get()?;

        #[derive(diesel::QueryableByName)]
        struct IdRow {
            #[diesel(sql_type = SqlUuid)]
            id: Uuid,
        }

        let claimed = sql_query(
            r#"
WITH candidate AS (
    SELECT oe.id
    FROM outbound_emails oe
    WHERE oe.id = $1
      AND (
        (oe.status = 'PENDING' AND (oe.scheduled_at IS NULL OR oe.scheduled_at <= NOW()))
        OR (oe.status = 'RUNNING' AND oe.lock_until IS NOT NULL AND oe.lock_until <= NOW())
      )
    FOR UPDATE SKIP LOCKED
)
UPDATE outbound_emails oe
SET status = 'RUNNING',
    lock_owner = $2,
    lock_until = NOW() + ($3 * INTERVAL '1 second'),
    updated_at = NOW()
FROM candidate
WHERE oe.id = candidate.id
RETURNING oe.id
            "#,
        )
        .bind::<SqlUuid, _>(id)
        .bind::<SqlUuid, _>(worker_id)
        .bind::<diesel::sql_types::BigInt, _>(EMAIL_RUN_LOCK_TTL_SECONDS)
        .get_result::<IdRow>(&mut conn)
        .optional()?;

        claimed
            .map(|row| OutboundEmail::find(row.id, pool))
            .transpose()
    }

    pub fn mark_sent(
        id: Uuid,
        provider_message_id: Option<String>,
        pool: &DbPool,
    ) -> Result<Self, PpdcError> {
        let mut conn = pool
            .get()?;
        diesel::update(outbound_emails::table.filter(outbound_emails::id.eq(id)))
            .set((
                outbound_emails::status.eq(OutboundEmailStatus::Sent.to_db()),
                outbound_emails::provider_message_id.eq(provider_message_id),
                outbound_emails::attempt_count.eq(outbound_emails::attempt_count + 1),
                outbound_emails::sent_at.eq(diesel::dsl::now),
                outbound_emails::last_error.eq::<Option<String>>(None),
                outbound_emails::lock_owner.eq::<Option<Uuid>>(None),
                outbound_emails::lock_until.eq::<Option<NaiveDateTime>>(None),
                outbound_emails::updated_at.eq(diesel::dsl::now),
            ))
            .execute(&mut conn)?;
        Self::find(id, pool)
    }

    pub fn mark_failed(id: Uuid, error: String, pool: &DbPool) -> Result<Self, PpdcError> {
        let mut conn = pool
            .get()?;
        diesel::update(outbound_emails::table.filter(outbound_emails::id.eq(id)))
            .set((
                outbound_emails::status.eq(OutboundEmailStatus::Failed.to_db()),
                outbound_emails::attempt_count.eq(outbound_emails::attempt_count + 1),
                outbound_emails::last_error.eq(Some(error)),
                outbound_emails::lock_owner.eq::<Option<Uuid>>(None),
                outbound_emails::lock_until.eq::<Option<NaiveDateTime>>(None),
                outbound_emails::updated_at.eq(diesel::dsl::now),
            ))
            .execute(&mut conn)?;
        Self::find(id, pool)
    }
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = crate::schema::outbound_emails)]
pub struct NewOutboundEmail {
    pub recipient_user_id: Option<Uuid>,
    pub reason: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub to_email: String,
    pub from_email: String,
    pub subject: String,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    pub status: String,
    pub provider: String,
    pub provider_message_id: Option<String>,
    pub attempt_count: i32,
    pub last_error: Option<String>,
    pub lock_owner: Option<Uuid>,
    pub lock_until: Option<NaiveDateTime>,
    pub scheduled_at: Option<NaiveDateTime>,
    pub sent_at: Option<NaiveDateTime>,
}

impl NewOutboundEmail {
    pub fn new(
        recipient_user_id: Option<Uuid>,
        reason: String,
        resource_type: Option<String>,
        resource_id: Option<Uuid>,
        to_email: String,
        from_email: String,
        subject: String,
        text_body: Option<String>,
        html_body: Option<String>,
        provider: OutboundEmailProvider,
        scheduled_at: Option<NaiveDateTime>,
    ) -> Self {
        Self {
            recipient_user_id,
            reason,
            resource_type,
            resource_id,
            to_email,
            from_email,
            subject,
            text_body,
            html_body,
            status: OutboundEmailStatus::Pending.to_db().to_string(),
            provider: provider.to_db().to_string(),
            provider_message_id: None,
            attempt_count: 0,
            last_error: None,
            lock_owner: None,
            lock_until: None,
            scheduled_at,
            sent_at: None,
        }
    }

    pub fn create(self, pool: &DbPool) -> Result<OutboundEmail, PpdcError> {
        let mut conn = pool
            .get()?;
        let email = diesel::insert_into(outbound_emails::table)
            .values(&self)
            .returning(OutboundEmail::as_returning())
            .get_result(&mut conn)?;
        Ok(email)
    }
}

async fn process_claimed_email(email: OutboundEmail, pool: &DbPool) -> Result<OutboundEmail, PpdcError> {
    if email.status_enum() != OutboundEmailStatus::Running {
        return Ok(email);
    }
    let id = email.id;

    let result = match email.provider_enum() {
        OutboundEmailProvider::Resend => send_email(SendEmailRequest {
            from: email.from_email.clone(),
            to: vec![email.to_email.clone()],
            subject: email.subject.clone(),
            text: email.text_body.clone(),
            html: email.html_body.clone(),
        })
        .await
        .map(|sent| sent.id),
    };

    match result {
        Ok(provider_message_id) => OutboundEmail::mark_sent(id, Some(provider_message_id), pool),
        Err(err) => OutboundEmail::mark_failed(id, err.message, pool),
    }
}

pub async fn process_pending_email(id: Uuid, pool: &DbPool) -> Result<OutboundEmail, PpdcError> {
    let worker_id = Uuid::new_v4();
    if let Some(email) = OutboundEmail::claim_if_due(id, worker_id, pool)? {
        return process_claimed_email(email, pool).await;
    }

    let email = OutboundEmail::find(id, pool)?;
    if email.status_enum() == OutboundEmailStatus::Running {
        return process_claimed_email(email, pool).await;
    }
    Ok(email)
}

pub async fn process_pending_emails(ids: Vec<Uuid>, pool: &DbPool) -> Result<(), PpdcError> {
    for id in ids {
        let _ = process_pending_email(id, pool).await?;
    }
    Ok(())
}
