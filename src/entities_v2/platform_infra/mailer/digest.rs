use std::collections::{HashMap, HashSet};

use axum::{debug_handler, extract::Extension, http::HeaderMap, Json};
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use serde::Serialize;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    journal::Journal,
    notification::successfully_pushed_publication_ids,
    post::{DigestVisiblePost, Post},
    trace_mention::TraceMention,
    user::{EmailNotificationMode, User, UserPrincipalType},
    user_post_state::UserPostState,
};
use crate::environment;
use crate::schema::{notification_digests, outbound_emails, users};

use super::{
    shared_journal_daily_digest_email, shared_journal_weekly_digest_email, NewOutboundEmail,
    OutboundEmail, OutboundEmailProvider, SharedJournalDigestEmailItem,
};

const SHARED_JOURNAL_DAILY_DIGEST_SEND_HOUR_LOCAL: u32 = 10;
const SHARED_JOURNAL_DAILY_DIGEST_REASON: &str = "SHARED_JOURNAL_ACTIVITY_DAILY_DIGEST";
const SHARED_JOURNAL_DAILY_DIGEST_KIND: &str = "SHARED_JOURNAL_ACTIVITY_DAILY";
const SHARED_JOURNAL_WEEKLY_DIGEST_REASON: &str = "SHARED_JOURNAL_ACTIVITY_WEEKLY_DIGEST";
const SHARED_JOURNAL_WEEKLY_DIGEST_KIND: &str = "SHARED_JOURNAL_ACTIVITY_WEEKLY";

fn shared_journal_weekly_digest_local_date(now_utc: DateTime<Utc>, tz: Tz) -> Option<NaiveDate> {
    let local_now = now_utc.with_timezone(&tz);
    if local_now.weekday() == chrono::Weekday::Mon
        && local_now.hour() < SHARED_JOURNAL_DAILY_DIGEST_SEND_HOUR_LOCAL
    {
        return None;
    }
    let monday =
        local_now.date_naive() - Duration::days(local_now.weekday().num_days_from_monday() as i64);
    Some(monday - Duration::days(7))
}

const NOTIFICATION_DIGEST_STATUS_EMPTY: &str = "EMPTY";
const NOTIFICATION_DIGEST_STATUS_ENQUEUED: &str = "ENQUEUED";
const SHARED_JOURNAL_DIGEST_MAX_ITEMS: usize = 8;

fn include_publication_in_digest(weekly: bool, seen: bool, pushed: bool, mentioned: bool) -> bool {
    if weekly {
        !seen
    } else {
        !pushed && !mentioned
    }
}

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

fn french_month_name(month: u32) -> &'static str {
    match month {
        1 => "janvier",
        2 => "février",
        3 => "mars",
        4 => "avril",
        5 => "mai",
        6 => "juin",
        7 => "juillet",
        8 => "août",
        9 => "septembre",
        10 => "octobre",
        11 => "novembre",
        12 => "décembre",
        _ => "",
    }
}

fn french_day_label(date: NaiveDate) -> String {
    format!("{} {}", date.day(), french_month_name(date.month()))
}

fn french_day_time_label(datetime: DateTime<Tz>) -> String {
    format!(
        "{} {} à {}",
        datetime.day(),
        french_month_name(datetime.month()),
        datetime.format("%H:%M")
    )
}

fn find_shared_journal_daily_digest_recipients(pool: &DbPool) -> Result<Vec<User>, PpdcError> {
    let mut conn = pool.get()?;
    let users = users::table
        .filter(users::principal_type.eq(UserPrincipalType::Human))
        .filter(
            users::shared_journal_activity_email_mode
                .eq(EmailNotificationMode::DailyDigest)
                .or(users::shared_journal_weekly_digest_enabled
                    .eq(true)
                    .and(users::shared_journal_activity_email_mode.ne(EmailNotificationMode::Off))),
        )
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
    digest_kind: &str,
    pool: &DbPool,
) -> Result<Option<Uuid>, PpdcError> {
    let mut conn = pool.get()?;
    let result = diesel::insert_into(notification_digests::table)
        .values(&NewNotificationDigest {
            id: Uuid::new_v4(),
            recipient_user_id,
            digest_kind: digest_kind.to_string(),
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
    digest_kind: &str,
    email_reason: &str,
    pool: &DbPool,
) -> Result<Option<Uuid>, PpdcError> {
    let mut conn = pool.get()?;
    let scheduled_at = Some(Utc::now().naive_utc());

    let result = conn.transaction::<Uuid, diesel::result::Error, _>(|conn| {
        let outbound_email = diesel::insert_into(outbound_emails::table)
            .values(&NewOutboundEmail::new(
                Some(recipient.id),
                email_reason.to_string(),
                None,
                None,
                recipient.email.clone(),
                environment::get_resend_from_email(),
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
                digest_kind: digest_kind.to_string(),
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
        let publishing_date_label = french_day_time_label(
            DateTime::<Utc>::from_naive_utc_and_offset(publishing_date, Utc)
                .with_timezone(&timezone),
        );

        items.push(SharedJournalDigestEmailItem {
            owner_display_name: owner.display_name(),
            journal_title: journal.title.clone(),
            journal_url: format!(
                "{}/me/journals/{}?post_id={}",
                app_base_url, journal.id, visible_post.post.id
            ),
            publishing_date_label,
            excerpt: build_trace_excerpt(&visible_post.content, 180),
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

fn create_shared_journal_digest_for_user(
    recipient: &User,
    local_date: NaiveDate,
    timezone: Tz,
    weekly: bool,
    pool: &DbPool,
) -> Result<DigestCreationResult, PpdcError> {
    let timezone_code = timezone.to_string();
    let (period_start, mut period_end) = local_day_bounds_utc(local_date, timezone)?;
    let digest_kind = if weekly {
        SHARED_JOURNAL_WEEKLY_DIGEST_KIND
    } else {
        SHARED_JOURNAL_DAILY_DIGEST_KIND
    };
    let email_reason = if weekly {
        SHARED_JOURNAL_WEEKLY_DIGEST_REASON
    } else {
        SHARED_JOURNAL_DAILY_DIGEST_REASON
    };
    {
        let mut conn = pool.get()?;
        let exists = notification_digests::table
            .filter(notification_digests::recipient_user_id.eq(recipient.id))
            .filter(notification_digests::digest_kind.eq(digest_kind))
            .filter(notification_digests::local_date.eq(local_date))
            .filter(notification_digests::timezone.eq(&timezone_code))
            .select(notification_digests::id)
            .first::<Uuid>(&mut conn)
            .optional()?
            .is_some();
        if exists {
            return Ok(DigestCreationResult::AlreadyExists);
        }
    }
    if weekly {
        period_end = local_midnight(timezone, local_date + Duration::days(7))?
            .with_timezone(&Utc)
            .naive_utc();
    }
    let visible_posts = Post::find_visible_shared_published_for_user_in_period(
        recipient.id,
        period_start,
        period_end,
        pool,
    )?;
    let trace_ids = visible_posts
        .iter()
        .filter_map(|visible_post| visible_post.post.source_trace_id)
        .collect::<Vec<_>>();
    let mentioned_trace_ids =
        TraceMention::find_active_trace_ids_for_user(recipient.id, &trace_ids, pool)?;
    let pushed_ids = if weekly {
        HashSet::new()
    } else {
        successfully_pushed_publication_ids(
            recipient.id,
            &visible_posts
                .iter()
                .map(|item| item.post.id)
                .collect::<Vec<_>>(),
            pool,
        )?
    };
    let seen_trace_ids = if weekly {
        UserPostState::find_last_seen_at_by_user_and_trace_ids(recipient.id, &trace_ids, pool)?
            .into_keys()
            .collect::<HashSet<_>>()
    } else {
        HashSet::new()
    };
    let visible_posts = visible_posts
        .into_iter()
        .filter(|visible_post| {
            let trace_id = visible_post.post.source_trace_id;
            include_publication_in_digest(
                weekly,
                trace_id
                    .map(|id| seen_trace_ids.contains(&id))
                    .unwrap_or(false),
                pushed_ids.contains(&visible_post.post.id),
                trace_id
                    .map(|id| mentioned_trace_ids.contains(&id))
                    .unwrap_or(false),
            )
        })
        .collect::<Vec<_>>();

    if visible_posts.is_empty() {
        return Ok(
            match create_empty_digest_row(
                recipient.id,
                local_date,
                &timezone_code,
                digest_kind,
                pool,
            )? {
                Some(digest_id) => DigestCreationResult::Empty(digest_id),
                None => DigestCreationResult::AlreadyExists,
            },
        );
    }

    let items =
        build_shared_journal_digest_items(recipient, local_date, timezone, visible_posts, pool)?;
    let template = if weekly {
        shared_journal_weekly_digest_email(
            &recipient.display_name(),
            &format!(
                "du {} au {}",
                french_day_label(local_date),
                french_day_label(local_date + Duration::days(6))
            ),
            items,
        )
    } else {
        shared_journal_daily_digest_email(
            &recipient.display_name(),
            &french_day_label(local_date),
            items,
        )
    };

    Ok(
        match create_enqueued_digest_row_with_email(
            recipient,
            local_date,
            &timezone_code,
            template.subject,
            template.text_body,
            template.html_body,
            digest_kind,
            email_reason,
            pool,
        )? {
            Some(digest_id) => DigestCreationResult::Enqueued(digest_id),
            None => DigestCreationResult::AlreadyExists,
        },
    )
}

/// Generates daily fallbacks and the latest completed weekly recap using the existing cron.
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
        for weekly in [false, true] {
            if weekly {
                if !recipient.shared_journal_weekly_digest_enabled
                    || recipient.shared_journal_activity_email_mode == EmailNotificationMode::Off
                {
                    continue;
                }
            } else if recipient.shared_journal_activity_email_mode
                != EmailNotificationMode::DailyDigest
            {
                continue;
            }
            let due_date = if weekly {
                shared_journal_weekly_digest_local_date(now, timezone)
            } else {
                shared_journal_daily_digest_local_date(now, timezone)
            };
            let Some(local_date) = due_date else {
                skipped.push(SharedJournalDigestGenerationSkip {
                    user_id: recipient.id,
                    reason: "not_due_yet".to_string(),
                });
                continue;
            };

            match create_shared_journal_digest_for_user(
                &recipient, local_date, timezone, weekly, pool,
            ) {
                Ok(DigestCreationResult::Empty(digest_id)) => empty_digest_ids.push(digest_id),
                Ok(DigestCreationResult::Enqueued(digest_id)) => {
                    enqueued_digest_ids.push(digest_id)
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daily_fallback_excludes_successful_pushes_and_separate_mentions() {
        assert!(include_publication_in_digest(false, false, false, false));
        assert!(!include_publication_in_digest(false, false, true, false));
        assert!(!include_publication_in_digest(false, false, false, true));
    }

    #[test]
    fn weekly_catchup_includes_pushed_and_mentioned_but_only_unread_items() {
        assert!(include_publication_in_digest(true, false, true, true));
        assert!(!include_publication_in_digest(true, true, true, true));
        assert!(!include_publication_in_digest(true, true, false, false));
    }

    #[test]
    fn weekly_schedule_uses_local_monday_ten_and_catches_up_after_missed_run() {
        let tz = chrono_tz::Europe::Paris;
        let before = DateTime::parse_from_rfc3339("2026-10-05T07:59:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(shared_journal_weekly_digest_local_date(before, tz), None);
        let due = before + Duration::minutes(1);
        let week_start = NaiveDate::from_ymd_opt(2026, 9, 28).unwrap();
        assert_eq!(
            shared_journal_weekly_digest_local_date(due, tz),
            Some(week_start)
        );
        assert_eq!(
            shared_journal_weekly_digest_local_date(due + Duration::days(2), tz),
            Some(week_start)
        );
    }

    #[test]
    fn weekly_bounds_follow_calendar_midnight_across_dst() {
        let tz = chrono_tz::Europe::Paris;
        let start = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
        let elapsed = local_midnight(tz, start + Duration::days(7))
            .unwrap()
            .signed_duration_since(local_midnight(tz, start).unwrap());
        assert_eq!(elapsed.num_hours(), 169);
    }
}
