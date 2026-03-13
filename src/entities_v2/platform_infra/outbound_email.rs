use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::schema::outbound_emails;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutboundEmailStatus {
    Pending,
    Sent,
    Failed,
}

impl OutboundEmailStatus {
    pub fn to_db(self) -> &'static str {
        match self {
            OutboundEmailStatus::Pending => "PENDING",
            OutboundEmailStatus::Sent => "SENT",
            OutboundEmailStatus::Failed => "FAILED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
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
        let mut conn = pool.get().expect("Failed to get a connection from the pool");
        let email = outbound_emails::table
            .select(Self::as_select())
            .filter(outbound_emails::id.eq(id))
            .first::<Self>(&mut conn)?;
        Ok(email)
    }

    pub fn list_due_pending(limit: i64, pool: &DbPool) -> Result<Vec<Self>, PpdcError> {
        let mut conn = pool.get().expect("Failed to get a connection from the pool");
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

    pub fn mark_sent(
        id: Uuid,
        provider_message_id: Option<String>,
        pool: &DbPool,
    ) -> Result<Self, PpdcError> {
        let mut conn = pool.get().expect("Failed to get a connection from the pool");
        diesel::update(outbound_emails::table.filter(outbound_emails::id.eq(id)))
            .set((
                outbound_emails::status.eq(OutboundEmailStatus::Sent.to_db()),
                outbound_emails::provider_message_id.eq(provider_message_id),
                outbound_emails::sent_at.eq(diesel::dsl::now),
                outbound_emails::last_error.eq::<Option<String>>(None),
                outbound_emails::updated_at.eq(diesel::dsl::now),
            ))
            .execute(&mut conn)?;
        Self::find(id, pool)
    }

    pub fn mark_failed(id: Uuid, error: String, pool: &DbPool) -> Result<Self, PpdcError> {
        let mut conn = pool.get().expect("Failed to get a connection from the pool");
        diesel::update(outbound_emails::table.filter(outbound_emails::id.eq(id)))
            .set((
                outbound_emails::status.eq(OutboundEmailStatus::Failed.to_db()),
                outbound_emails::attempt_count.eq(outbound_emails::attempt_count + 1),
                outbound_emails::last_error.eq(Some(error)),
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
            scheduled_at,
            sent_at: None,
        }
    }

    pub fn create(self, pool: &DbPool) -> Result<OutboundEmail, PpdcError> {
        let mut conn = pool.get().expect("Failed to get a connection from the pool");
        let email = diesel::insert_into(outbound_emails::table)
            .values(&self)
            .returning(OutboundEmail::as_returning())
            .get_result(&mut conn)?;
        Ok(email)
    }
}
