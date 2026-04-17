use std::collections::{HashMap, HashSet};

use axum::{debug_handler, extract::Extension, http::HeaderMap, Json};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use serde::Serialize;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    journal::Journal,
    post::{DigestVisiblePost, Post},
    user::{EmailNotificationMode, User, UserPrincipalType},
};
use crate::environment;
use crate::schema::{notification_digests, outbound_emails, users};

use super::{
    shared_journal_daily_digest_email, NewOutboundEmail, OutboundEmail, OutboundEmailProvider,
    SharedJournalDigestEmailItem,
};

const SHARED_JOURNAL_DAILY_DIGEST_SEND_HOUR_LOCAL: u32 = 10;
const SHARED_JOURNAL_DAILY_DIGEST_REASON: &str = "SHARED_JOURNAL_ACTIVITY_DAILY_DIGEST";
const SHARED_JOURNAL_DAILY_DIGEST_KIND: &str = "SHARED_JOURNAL_ACTIVITY_DAILY";
const NOTIFICATION_DIGEST_STATUS_EMPTY: &str = "EMPTY";
const NOTIFICATION_DIGEST_STATUS_ENQUEUED: &str = "ENQUEUED";
const SHARED_JOURNAL_DIGEST_MAX_ITEMS: usize = 8;

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = crate::schema::notification_digests)]
struct NewNotificationDigest {
    pub id: Uuid,
    pub recipient_user_id: Uuid,
    pub digest_kind: String,
    pub local_date: NaiveDate,
    pub timezone: String,
    pub status: String,
    pub outbound_email_id: Option<Uuid>,
}

#[derive(Serialize)]
pub struct SharedJournalDigestGenerationSkip {
    pub user_id: Uuid,
    pub reason: String,
}

#[derive(Serialize)]
pub struct SharedJournalDigestGenerationError {
    pub user_id: Uuid,
    pub message: String,
}

#[derive(Serialize)]
pub struct SharedJournalDailyDigestsGenerationResponse {
    pub candidate_user_ids: Vec<Uuid>,
    pub enqueued_digest_ids: Vec<Uuid>,
    pub empty_digest_ids: Vec<Uuid>,
    pub skipped: Vec<SharedJournalDigestGenerationSkip>,
    pub failed: Vec<SharedJournalDigestGenerationError>,
}

enum DigestCreationResult {
    Empty(Uuid),
    Enqueued(Uuid),
    AlreadyExists,
}

fn build_trace_excerpt(content: &str, max_chars: usize) -> String {
    let excerpt = content.trim().chars().take(max_chars).collect::<String>();
    if content.trim().chars().count() > max_chars {
        format!("{}...", excerpt)
    } else {
        excerpt
    }
}

fn local_midnight(tz: Tz, date: NaiveDate) -> Result<DateTime<Tz>, PpdcError> {
    let naive_midnight = date.and_hms_opt(0, 0, 0).ok_or_else(|| {
        PpdcError::new(
            500,
            ErrorType::InternalError,
            "Failed to construct local midnight".to_string(),
        )
    })?;

    tz.from_local_datetime(&naive_midnight)
        .single()
        .or_else(|| tz.from_local_datetime(&naive_midnight).earliest())
        .or_else(|| tz.from_local_datetime(&naive_midnight).latest())
        .ok_or_else(|| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                "Failed to resolve local midnight for timezone".to_string(),
            )
        })
}

fn parse_user_timezone_or_utc(user: &User) -> Tz {
    user.timezone.parse::<Tz>().unwrap_or(chrono_tz::UTC)
}

fn shared_journal_daily_digest_local_date(now_utc: DateTime<Utc>, tz: Tz) -> Option<NaiveDate> {
    let local_now = now_utc.with_timezone(&tz);
    if local_now.hour() < SHARED_JOURNAL_DAILY_DIGEST_SEND_HOUR_LOCAL {
        return None;
    }

    Some(local_now.date_naive() - Duration::days(1))
}

fn local_day_bounds_utc(
    local_date: NaiveDate,
    tz: Tz,
) -> Result<(NaiveDateTime, NaiveDateTime), PpdcError> {
    let start = local_midnight(tz, local_date)?;
    let end = local_midnight(tz, local_date + Duration::days(1))?;
    Ok((
        start.with_timezone(&Utc).naive_utc(),
        end.with_timezone(&Utc).naive_utc(),
    ))
}

fn find_shared_journal_daily_digest_recipients(pool: &DbPool) -> Result<Vec<User>, PpdcError> {
    let mut conn = pool.get()?;
    let users = users::table
        .filter(users::principal_type.eq(UserPrincipalType::Human))
        .filter(users::shared_journal_activity_email_mode.eq(EmailNotificationMode::DailyDigest))
        .select(User::as_select())
        .load::<User>(&mut conn)?;

    Ok(users
        .into_iter()
        .filter(|user| !user.email.trim().is_empty())
        .collect())
}

fn create_empty_digest_row(
    recipient_user_id: Uuid,
    local_date: NaiveDate,
    timezone: &str,
    pool: &DbPool,
) -> Result<Option<Uuid>, PpdcError> {
    let mut conn = pool.get()?;
    let result = diesel::insert_into(notification_digests::table)
        .values(&NewNotificationDigest {
            id: Uuid::new_v4(),
            recipient_user_id,
            digest_kind: SHARED_JOURNAL_DAILY_DIGEST_KIND.to_string(),
            local_date,
            timezone: timezone.to_string(),
            status: NOTIFICATION_DIGEST_STATUS_EMPTY.to_string(),
            outbound_email_id: None,
        })
        .returning(notification_digests::id)
        .get_result::<Uuid>(&mut conn);

    match result {
        Ok(digest) => Ok(Some(digest)),
        Err(DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn create_enqueued_digest_row_with_email(
    recipient: &User,
    local_date: NaiveDate,
    timezone: &str,
    subject: String,
    text_body: Option<String>,
    html_body: Option<String>,
    pool: &DbPool,
) -> Result<Option<Uuid>, PpdcError> {
    let mut conn = pool.get()?;
    let scheduled_at = Some(Utc::now().naive_utc());

    let result = conn.transaction::<Uuid, diesel::result::Error, _>(|conn| {
        let outbound_email = diesel::insert_into(outbound_emails::table)
            .values(&NewOutboundEmail::new(
                Some(recipient.id),
                SHARED_JOURNAL_DAILY_DIGEST_REASON.to_string(),
                None,
                None,
                recipient.email.clone(),
                "Matière Grise <noreply@ppdcoeur.fr>".to_string(),
                subject,
                text_body,
                html_body,
                OutboundEmailProvider::Resend,
                scheduled_at,
            ))
            .returning(OutboundEmail::as_returning())
            .get_result::<OutboundEmail>(conn)?;

        diesel::insert_into(notification_digests::table)
            .values(&NewNotificationDigest {
                id: Uuid::new_v4(),
                recipient_user_id: recipient.id,
                digest_kind: SHARED_JOURNAL_DAILY_DIGEST_KIND.to_string(),
                local_date,
                timezone: timezone.to_string(),
                status: NOTIFICATION_DIGEST_STATUS_ENQUEUED.to_string(),
                outbound_email_id: Some(outbound_email.id),
            })
            .returning(notification_digests::id)
            .get_result::<Uuid>(conn)
    });

    match result {
        Ok(digest) => Ok(Some(digest)),
        Err(DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn build_shared_journal_digest_items(
    recipient: &User,
    local_date: NaiveDate,
    timezone: Tz,
    visible_posts: Vec<DigestVisiblePost>,
    pool: &DbPool,
) -> Result<Vec<SharedJournalDigestEmailItem>, PpdcError> {
    let journal_ids = visible_posts
        .iter()
        .map(|visible_post| visible_post.journal_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let owner_ids = visible_posts
        .iter()
        .map(|visible_post| visible_post.post.user_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let journals = Journal::find_many(journal_ids, pool)?
        .into_iter()
        .map(|journal| (journal.id, journal))
        .collect::<HashMap<_, _>>();
    let owners = User::find_many(&owner_ids, pool)?
        .into_iter()
        .map(|user| (user.id, user))
        .collect::<HashMap<_, _>>();

    let app_base_url = environment::get_app_base_url()
        .trim_end_matches('/')
        .to_string();
    let mut items = Vec::new();

    for visible_post in visible_posts
        .into_iter()
        .take(SHARED_JOURNAL_DIGEST_MAX_ITEMS)
    {
        let Some(journal) = journals.get(&visible_post.journal_id) else {
            continue;
        };
        let Some(owner) = owners.get(&visible_post.post.user_id) else {
            continue;
        };
        let publishing_date = visible_post
            .post
            .publishing_date
            .unwrap_or(visible_post.post.created_at);
        let publishing_date_label =
            DateTime::<Utc>::from_naive_utc_and_offset(publishing_date, Utc)
                .with_timezone(&timezone)
                .format("%d/%m/%Y à %H:%M")
                .to_string();

        items.push(SharedJournalDigestEmailItem {
            owner_display_name: owner.display_name(),
            journal_title: journal.title.clone(),
            journal_url: format!(
                "{}/me/journals/{}?post_id={}",
                app_base_url, journal.id, visible_post.post.id
            ),
            publishing_date_label,
            excerpt: build_trace_excerpt(&visible_post.post.content, 180),
        });
    }

    if items.is_empty() {
        return Err(PpdcError::new(
            500,
            ErrorType::InternalError,
            format!(
                "No digest items could be built for user {} on {}",
                recipient.id, local_date
            ),
        ));
    }

    Ok(items)
}

fn create_shared_journal_daily_digest_for_user(
    recipient: &User,
    local_date: NaiveDate,
    timezone: Tz,
    pool: &DbPool,
) -> Result<DigestCreationResult, PpdcError> {
    let timezone_code = timezone.to_string();
    let (period_start, period_end) = local_day_bounds_utc(local_date, timezone)?;
    let visible_posts = Post::find_visible_shared_published_for_user_in_period(
        recipient.id,
        period_start,
        period_end,
        pool,
    )?;

    if visible_posts.is_empty() {
        return Ok(
            match create_empty_digest_row(recipient.id, local_date, &timezone_code, pool)? {
                Some(digest_id) => DigestCreationResult::Empty(digest_id),
                None => DigestCreationResult::AlreadyExists,
            },
        );
    }

    let items =
        build_shared_journal_digest_items(recipient, local_date, timezone, visible_posts, pool)?;
    let template = shared_journal_daily_digest_email(
        &recipient.display_name(),
        &local_date.format("%d/%m/%Y").to_string(),
        items,
    );

    Ok(
        match create_enqueued_digest_row_with_email(
            recipient,
            local_date,
            &timezone_code,
            template.subject,
            template.text_body,
            template.html_body,
            pool,
        )? {
            Some(digest_id) => DigestCreationResult::Enqueued(digest_id),
            None => DigestCreationResult::AlreadyExists,
        },
    )
}

/// Generates the daily shared-journal digest emails for users who opted in and already reached the local send hour.
pub fn generate_shared_journal_daily_digests(
    pool: &DbPool,
) -> Result<SharedJournalDailyDigestsGenerationResponse, PpdcError> {
    let recipients = find_shared_journal_daily_digest_recipients(pool)?;
    let candidate_user_ids = recipients.iter().map(|user| user.id).collect::<Vec<_>>();
    let now = Utc::now();
    let mut enqueued_digest_ids = Vec::new();
    let mut empty_digest_ids = Vec::new();
    let mut skipped = Vec::new();
    let mut failed = Vec::new();

    for recipient in recipients {
        let timezone = parse_user_timezone_or_utc(&recipient);
        let Some(local_date) = shared_journal_daily_digest_local_date(now, timezone) else {
            skipped.push(SharedJournalDigestGenerationSkip {
                user_id: recipient.id,
                reason: "not_due_yet".to_string(),
            });
            continue;
        };

        match create_shared_journal_daily_digest_for_user(&recipient, local_date, timezone, pool) {
            Ok(DigestCreationResult::Empty(digest_id)) => empty_digest_ids.push(digest_id),
            Ok(DigestCreationResult::Enqueued(digest_id)) => enqueued_digest_ids.push(digest_id),
            Ok(DigestCreationResult::AlreadyExists) => {
                skipped.push(SharedJournalDigestGenerationSkip {
                    user_id: recipient.id,
                    reason: "already_generated".to_string(),
                });
            }
            Err(error) => failed.push(SharedJournalDigestGenerationError {
                user_id: recipient.id,
                message: error.message,
            }),
        }
    }

    Ok(SharedJournalDailyDigestsGenerationResponse {
        candidate_user_ids,
        enqueued_digest_ids,
        empty_digest_ids,
        skipped,
        failed,
    })
}

#[debug_handler]
pub async fn post_generate_shared_journal_daily_digests_route(
    Extension(pool): Extension<DbPool>,
    headers: HeaderMap,
) -> Result<Json<SharedJournalDailyDigestsGenerationResponse>, PpdcError> {
    let provided_token = headers
        .get("x-internal-cron-token")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(PpdcError::unauthorized)?;

    if provided_token != environment::get_internal_cron_token() {
        return Err(PpdcError::unauthorized());
    }

    Ok(Json(generate_shared_journal_daily_digests(&pool)?))
}
