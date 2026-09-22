use chrono::Utc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    analysis_summary::AnalysisSummary,
    error::PpdcError,
    landscape_analysis::LandscapeAnalysis,
    message::Message,
    post::{Post, PostStatus},
    post_grant::PostGrant,
    relationship::Relationship,
    trace::Trace,
    user::{User, UserPrincipalType},
};
use crate::environment;

use super::{
    mailer::{self, NewOutboundEmail, OutboundEmailProvider},
    push,
};

fn enqueue_received_message_notification_email(
    message: &Message,
    pool: &DbPool,
) -> Result<Option<Uuid>, PpdcError> {
    let sender = User::find(&message.sender_user_id, pool)?;
    let recipient = User::find(&message.recipient_user_id, pool)?;
    if sender.principal_type != UserPrincipalType::Human
        || recipient.principal_type != UserPrincipalType::Human
        || recipient.email.trim().is_empty()
        || sender.id == recipient.id
        || !recipient.allows_instant_received_message_email()
    {
        return Ok(None);
    }

    let conversation_url = if let Some(post_id) = message.post_id {
        let post = Post::find_full(post_id, pool)?;
        let source_trace = post
            .source_trace_id
            .map(|trace_id| Trace::find_full_trace(trace_id, pool))
            .transpose()?;
        let source_journal_id = source_trace.as_ref().and_then(|trace| trace.journal_id);
        let sender_is_owner = sender.id == post.user_id;

        if sender_is_owner {
            source_journal_id.map(|journal_id| {
                format!(
                    "{}/me/conversation?journal_id={}&post_id={}&view=post_chat&recipient_user_id={}",
                    environment::get_app_base_url().trim_end_matches('/'),
                    journal_id,
                    post_id,
                    sender.id
                )
            })
        } else {
            source_trace
                .and_then(|trace| {
                    trace.journal_id.map(|journal_id| {
                        format!(
                            "{}/me/conversation?journal_id={}&trace_id={}&view=trace_chat&recipient_user_id={}",
                            environment::get_app_base_url().trim_end_matches('/'),
                            journal_id,
                            trace.id,
                            sender.id
                        )
                    })
                })
                .or_else(|| {
                    source_journal_id.map(|journal_id| {
                        format!(
                            "{}/me/conversation?journal_id={}&post_id={}&view=post_chat&recipient_user_id={}",
                            environment::get_app_base_url().trim_end_matches('/'),
                            journal_id,
                            post_id,
                            sender.id
                        )
                    })
                })
        }
    } else {
        message
            .trace_id
            .as_ref()
            .map(|trace_id| {
                Trace::find_full_trace(*trace_id, pool)
                    .ok()
                    .and_then(|trace| {
                        trace.journal_id.map(|journal_id| {
                            format!(
                                "{}/me/conversation?journal_id={}&trace_id={}&view=trace_chat&recipient_user_id={}",
                                environment::get_app_base_url().trim_end_matches('/'),
                                journal_id,
                                trace_id,
                                sender.id
                            )
                        })
                    })
            })
            .flatten()
    };

    let template = mailer::message_received_email(
        &recipient.display_name(),
        &sender.display_name(),
        &message.content,
        conversation_url.as_deref(),
    );
    let email = NewOutboundEmail::new(
        Some(recipient.id),
        "USER_MESSAGE_RECEIVED".to_string(),
        Some("MESSAGE".to_string()),
        Some(message.id),
        recipient.email,
        crate::environment::get_resend_from_email(),
        template.subject,
        template.text_body,
        template.html_body,
        OutboundEmailProvider::Resend,
        Some(Utc::now().naive_utc()),
    )
    .create(pool)?;

    Ok(Some(email.id))
}

fn notification_preview(content: &str, max_chars: usize) -> String {
    let trimmed = content.trim();
    let preview = trimmed.chars().take(max_chars).collect::<String>();
    if trimmed.chars().count() > max_chars {
        format!("{}...", preview)
    } else {
        preview
    }
}

fn enqueue_weekly_recap_feedback_email(
    analysis: &LandscapeAnalysis,
    summary: &AnalysisSummary,
    feedback: &Message,
    pool: &DbPool,
) -> Result<Option<Uuid>, PpdcError> {
    let recipient = User::find(&analysis.user_id, pool)?;
    if recipient.email.trim().is_empty() || !recipient.allows_mentor_feedback_email() {
        return Ok(None);
    }
    let mentor = User::find(&feedback.sender_user_id, pool)?;
    let recap_url = format!(
        "{}/me/home?feedback=open&analysis_id={}",
        environment::get_app_base_url().trim_end_matches('/'),
        analysis.id
    );
    let template = mailer::weekly_recap_email(
        &recipient.display_name(),
        &mentor.display_name(),
        &feedback.title,
        &notification_preview(&feedback.content, 220),
        &summary.title,
        &notification_preview(&summary.short_content, 180),
        &recap_url,
    );
    let email = NewOutboundEmail::new(
        Some(recipient.id),
        "WEEKLY_RECAP_FEEDBACK_EMAIL".to_string(),
        Some("LANDSCAPE_ANALYSIS".to_string()),
        Some(analysis.id),
        recipient.email,
        crate::environment::get_resend_from_email(),
        template.subject,
        template.text_body,
        template.html_body,
        OutboundEmailProvider::Resend,
        Some(Utc::now().naive_utc()),
    )
    .create(pool)?;
    Ok(Some(email.id))
}

pub fn spawn_message_received_notification(message: Message, pool: DbPool) {
    if message.sender_user_id == message.recipient_user_id {
        return;
    }

    tokio::spawn(async move {
        let mut push_sent = false;
        match push::message_received_notification(&message, &pool).await {
            Ok(notification) => {
                match push::send_to_user(message.recipient_user_id, notification, &pool).await {
                    Ok(result) => {
                        push_sent = result.any_sent();
                        info!(
                            target: "notification",
                            message_id = %message.id,
                            recipient_user_id = %message.recipient_user_id,
                            push_attempted_count = result.attempted_count,
                            push_sent_count = result.sent_count,
                            "message_received_push_dispatch_completed"
                        );
                    }
                    Err(err) => {
                        warn!(
                            target: "notification",
                            message_id = %message.id,
                            recipient_user_id = %message.recipient_user_id,
                            error = %err.message,
                            "message_received_push_dispatch_failed"
                        );
                    }
                }
            }
            Err(err) => {
                warn!(
                    target: "notification",
                    message_id = %message.id,
                    recipient_user_id = %message.recipient_user_id,
                    error = %err.message,
                    "message_received_push_build_failed"
                );
            }
        }

        if push_sent {
            return;
        }

        match enqueue_received_message_notification_email(&message, &pool) {
            Ok(Some(email_id)) => {
                if let Err(err) = mailer::process_pending_emails(vec![email_id], &pool).await {
                    warn!(
                        target: "notification",
                        message_id = %message.id,
                        recipient_user_id = %message.recipient_user_id,
                        email_id = %email_id,
                        error = %err.message,
                        "message_received_email_processing_failed"
                    );
                }
            }
            Ok(None) => {}
            Err(err) => {
                warn!(
                    target: "notification",
                    message_id = %message.id,
                    recipient_user_id = %message.recipient_user_id,
                    error = %err.message,
                    "message_received_email_enqueue_failed"
                );
            }
        }
    });
}

pub fn spawn_weekly_recap_feedback_notification(
    analysis: LandscapeAnalysis,
    summary: AnalysisSummary,
    feedback: Message,
    pool: DbPool,
) {
    tokio::spawn(async move {
        let push_sent = match push::message_received_notification(&feedback, &pool).await {
            Ok(notification) => {
                match push::send_to_user(feedback.recipient_user_id, notification, &pool).await {
                    Ok(result) => {
                        info!(
                            target: "notification",
                            analysis_id = %analysis.id,
                            message_id = %feedback.id,
                            recipient_user_id = %feedback.recipient_user_id,
                            push_attempted_count = result.attempted_count,
                            push_sent_count = result.sent_count,
                            "weekly_recap_feedback_push_dispatch_completed"
                        );
                        result.any_sent()
                    }
                    Err(err) => {
                        warn!(
                            target: "notification",
                            analysis_id = %analysis.id,
                            message_id = %feedback.id,
                            recipient_user_id = %feedback.recipient_user_id,
                            error = %err.message,
                            "weekly_recap_feedback_push_dispatch_failed"
                        );
                        false
                    }
                }
            }
            Err(err) => {
                warn!(
                    target: "notification",
                    analysis_id = %analysis.id,
                    message_id = %feedback.id,
                    error = %err.message,
                    "weekly_recap_feedback_push_build_failed"
                );
                false
            }
        };

        if push_sent {
            return;
        }

        match enqueue_weekly_recap_feedback_email(&analysis, &summary, &feedback, &pool) {
            Ok(Some(email_id)) => {
                if let Err(err) = mailer::process_pending_emails(vec![email_id], &pool).await {
                    warn!(
                        target: "notification",
                        analysis_id = %analysis.id,
                        message_id = %feedback.id,
                        email_id = %email_id,
                        error = %err.message,
                        "weekly_recap_feedback_email_processing_failed"
                    );
                }
            }
            Ok(None) => {
                info!(
                    target: "notification",
                    analysis_id = %analysis.id,
                    message_id = %feedback.id,
                    recipient_user_id = %feedback.recipient_user_id,
                    "weekly_recap_feedback_email_skipped"
                );
            }
            Err(err) => {
                warn!(
                    target: "notification",
                    analysis_id = %analysis.id,
                    message_id = %feedback.id,
                    recipient_user_id = %feedback.recipient_user_id,
                    error = %err.message,
                    "weekly_recap_feedback_email_enqueue_failed"
                );
            }
        }
    });
}

pub fn spawn_follow_request_received_push_notification(relationship: Relationship, pool: DbPool) {
    tokio::spawn(async move {
        let notification =
            match push::follow_request_received_notification(&relationship, &pool).await {
                Ok(notification) => notification,
                Err(err) => {
                    warn!(
                        target: "notification",
                        relationship_id = %relationship.id,
                        error = %err.message,
                        "follow_request_received_push_build_failed"
                    );
                    return;
                }
            };

        match push::send_to_user(relationship.target_user_id, notification, &pool).await {
            Ok(result) => info!(
                target: "notification",
                relationship_id = %relationship.id,
                recipient_user_id = %relationship.target_user_id,
                push_attempted_count = result.attempted_count,
                push_sent_count = result.sent_count,
                "follow_request_received_push_dispatch_completed"
            ),
            Err(err) => warn!(
                target: "notification",
                relationship_id = %relationship.id,
                recipient_user_id = %relationship.target_user_id,
                error = %err.message,
                "follow_request_received_push_dispatch_failed"
            ),
        }
    });
}

pub fn spawn_follow_request_accepted_push_notification(relationship: Relationship, pool: DbPool) {
    tokio::spawn(async move {
        let notification =
            match push::follow_request_accepted_notification(&relationship, &pool).await {
                Ok(notification) => notification,
                Err(err) => {
                    warn!(
                        target: "notification",
                        relationship_id = %relationship.id,
                        error = %err.message,
                        "follow_request_accepted_push_build_failed"
                    );
                    return;
                }
            };

        match push::send_to_user(relationship.requester_user_id, notification, &pool).await {
            Ok(result) => info!(
                target: "notification",
                relationship_id = %relationship.id,
                recipient_user_id = %relationship.requester_user_id,
                push_attempted_count = result.attempted_count,
                push_sent_count = result.sent_count,
                "follow_request_accepted_push_dispatch_completed"
            ),
            Err(err) => warn!(
                target: "notification",
                relationship_id = %relationship.id,
                recipient_user_id = %relationship.requester_user_id,
                error = %err.message,
                "follow_request_accepted_push_dispatch_failed"
            ),
        }
    });
}

pub fn spawn_post_published_push_notification(
    post: Post,
    excluded_recipient_ids: Vec<Uuid>,
    pool: DbPool,
) {
    if post.status != PostStatus::Published || post.source_ref().is_none() {
        return;
    }

    tokio::spawn(async move {
        let notification = match push::post_published_notification(&post, &pool).await {
            Ok(Some(notification)) => notification,
            Ok(None) => return,
            Err(err) => {
                warn!(
                    target: "notification",
                    post_id = %post.id,
                    error = %err.message,
                    "post_published_push_build_failed"
                );
                return;
            }
        };

        let recipient_ids = match PostGrant::find_active_recipient_user_ids_for_post(&post, &pool) {
            Ok(recipient_ids) => recipient_ids,
            Err(err) => {
                warn!(
                    target: "notification",
                    post_id = %post.id,
                    error = %err.message,
                    "post_published_push_recipient_lookup_failed"
                );
                return;
            }
        };
        if recipient_ids.is_empty() {
            return;
        }

        let recipients = match User::find_many(&recipient_ids, &pool) {
            Ok(recipients) => recipients,
            Err(err) => {
                warn!(
                    target: "notification",
                    post_id = %post.id,
                    error = %err.message,
                    "post_published_push_recipient_user_lookup_failed"
                );
                return;
            }
        };

        for recipient in recipients.into_iter().filter(|recipient| {
            recipient.id != post.user_id
                && recipient.principal_type == UserPrincipalType::Human
                && !excluded_recipient_ids.contains(&recipient.id)
        }) {
            match push::send_to_mobile_user(recipient.id, notification.clone(), &pool).await {
                Ok(result) => {
                    info!(
                        target: "notification",
                        post_id = %post.id,
                        recipient_user_id = %recipient.id,
                        push_attempted_count = result.attempted_count,
                        push_sent_count = result.sent_count,
                        "post_published_push_dispatch_completed"
                    );
                }
                Err(err) => {
                    warn!(
                        target: "notification",
                        post_id = %post.id,
                        recipient_user_id = %recipient.id,
                        error = %err.message,
                        "post_published_push_dispatch_failed"
                    );
                }
            }
        }
    });
}

pub fn spawn_trace_mention_push_notification(post: Post, recipient_ids: Vec<Uuid>, pool: DbPool) {
    if post.status != PostStatus::Published
        || post.source_trace_id.is_none()
        || recipient_ids.is_empty()
    {
        return;
    }

    tokio::spawn(async move {
        let notification = match push::trace_mention_notification(&post, &pool).await {
            Ok(Some(notification)) => notification,
            Ok(None) => return,
            Err(err) => {
                warn!(
                    target: "notification",
                    post_id = %post.id,
                    error = %err.message,
                    "trace_mention_push_build_failed"
                );
                return;
            }
        };

        let recipients = match User::find_many(&recipient_ids, &pool) {
            Ok(recipients) => recipients,
            Err(err) => {
                warn!(
                    target: "notification",
                    post_id = %post.id,
                    error = %err.message,
                    "trace_mention_push_recipient_user_lookup_failed"
                );
                return;
            }
        };

        for recipient in recipients.into_iter().filter(|recipient| {
            recipient.id != post.user_id && recipient.principal_type == UserPrincipalType::Human
        }) {
            match push::send_to_mobile_user(recipient.id, notification.clone(), &pool).await {
                Ok(result) => info!(
                    target: "notification",
                    post_id = %post.id,
                    recipient_user_id = %recipient.id,
                    push_attempted_count = result.attempted_count,
                    push_sent_count = result.sent_count,
                    "trace_mention_push_dispatch_completed"
                ),
                Err(err) => warn!(
                    target: "notification",
                    post_id = %post.id,
                    recipient_user_id = %recipient.id,
                    error = %err.message,
                    "trace_mention_push_dispatch_failed"
                ),
            }
        }
    });
}

pub fn spawn_journal_history_shared_push_notification(
    journal_id: Uuid,
    owner_user_id: Uuid,
    recipient_user_id: Uuid,
    shared_trace_count: usize,
    pool: DbPool,
) {
    if shared_trace_count == 0 || owner_user_id == recipient_user_id {
        return;
    }

    tokio::spawn(async move {
        let recipient = match User::find(&recipient_user_id, &pool) {
            Ok(recipient) if recipient.principal_type == UserPrincipalType::Human => recipient,
            Ok(_) => return,
            Err(err) => {
                warn!(
                    target: "notification",
                    journal_id = %journal_id,
                    recipient_user_id = %recipient_user_id,
                    error = %err.message,
                    "journal_history_shared_push_recipient_lookup_failed"
                );
                return;
            }
        };
        let journal = match crate::entities_v2::journal::Journal::find_full(journal_id, &pool) {
            Ok(journal) => journal,
            Err(err) => {
                warn!(
                    target: "notification",
                    journal_id = %journal_id,
                    error = %err.message,
                    "journal_history_shared_push_journal_lookup_failed"
                );
                return;
            }
        };
        let owner = match User::find(&owner_user_id, &pool) {
            Ok(owner) => owner,
            Err(err) => {
                warn!(
                    target: "notification",
                    journal_id = %journal_id,
                    owner_user_id = %owner_user_id,
                    error = %err.message,
                    "journal_history_shared_push_owner_lookup_failed"
                );
                return;
            }
        };
        let notification =
            push::journal_history_shared_notification(&journal, &owner, shared_trace_count, &pool)
                .await;

        match push::send_to_mobile_user(recipient.id, notification, &pool).await {
            Ok(result) => info!(
                target: "notification",
                journal_id = %journal_id,
                recipient_user_id = %recipient.id,
                shared_trace_count,
                push_attempted_count = result.attempted_count,
                push_sent_count = result.sent_count,
                "journal_history_shared_push_dispatch_completed"
            ),
            Err(err) => warn!(
                target: "notification",
                journal_id = %journal_id,
                recipient_user_id = %recipient.id,
                shared_trace_count,
                error = %err.message,
                "journal_history_shared_push_dispatch_failed"
            ),
        }
    });
}
