use chrono::Utc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::PpdcError,
    message::Message,
    post::Post,
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
        "hupo <noreply@ppdcoeur.fr>".to_string(),
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
        match push::message_received_notification(&message, &pool) {
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
