use chrono::NaiveDateTime;

use crate::environment;

#[derive(Debug, Clone)]
pub struct EmailTemplate {
    pub subject: String,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
}

const SHARED_TRACE_FINALIZED_TEXT: &str = include_str!("templates/shared_trace_finalized.txt");
const SHARED_TRACE_FINALIZED_HTML: &str = include_str!("templates/shared_trace_finalized.html");
const MESSAGE_RECEIVED_TEXT: &str = include_str!("templates/message_received.txt");
const MESSAGE_RECEIVED_HTML: &str = include_str!("templates/message_received.html");
const FOLLOW_REQUEST_RECEIVED_TEXT: &str = include_str!("templates/follow_request_received.txt");
const FOLLOW_REQUEST_RECEIVED_HTML: &str = include_str!("templates/follow_request_received.html");
const PASSWORD_RESET_TEXT: &str = include_str!("templates/password_reset.txt");
const PASSWORD_RESET_HTML: &str = include_str!("templates/password_reset.html");
const NEW_USER_SIGNUP_TEXT: &str = include_str!("templates/new_user_signup.txt");
const NEW_USER_SIGNUP_HTML: &str = include_str!("templates/new_user_signup.html");
const JOURNAL_ACCESS_GRANTED_TEXT: &str = include_str!("templates/journal_access_granted.txt");
const JOURNAL_ACCESS_GRANTED_HTML: &str = include_str!("templates/journal_access_granted.html");
const DAILY_RECAP_FEEDBACK_TEXT: &str = include_str!("templates/daily_recap_feedback.txt");
const DAILY_RECAP_FEEDBACK_HTML: &str = include_str!("templates/daily_recap_feedback.html");
const SHARED_JOURNAL_DAILY_DIGEST_TEXT: &str =
    include_str!("templates/shared_journal_daily_digest.txt");
const SHARED_JOURNAL_DAILY_DIGEST_HTML: &str =
    include_str!("templates/shared_journal_daily_digest.html");

#[derive(Debug, Clone)]
pub struct SharedJournalDigestEmailItem {
    pub owner_display_name: String,
    pub journal_title: String,
    pub journal_url: String,
    pub publishing_date_label: String,
    pub excerpt: String,
}

fn build_trace_excerpt(content: &str, max_chars: usize) -> String {
    let excerpt = content.trim().chars().take(max_chars).collect::<String>();
    if content.trim().chars().count() > max_chars {
        format!("{}...", excerpt)
    } else {
        excerpt
    }
}

fn distinct_owner_display_names(items: &[SharedJournalDigestEmailItem]) -> Vec<String> {
    let mut owners = Vec::new();
    for item in items {
        if !owners.contains(&item.owner_display_name) {
            owners.push(item.owner_display_name.clone());
        }
    }
    owners
}

fn join_owner_display_names(owners: &[String]) -> String {
    owners.join(", ")
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn render_template(template: &str, variables: &[(&str, String)]) -> String {
    let mut rendered = template.to_string();
    for (key, value) in variables {
        rendered = rendered.replace(&format!("{{{{{}}}}}", key), value);
    }
    rendered
}

fn append_contact_preferences_footer(template: EmailTemplate) -> EmailTemplate {
    let preferences_url = format!(
        "{}/me/user#email-preferences",
        environment::get_app_base_url().trim_end_matches('/')
    );
    let footer_title = "Réglez vos préférences de contact par mail";

    let text_body = template.text_body.map(|body| {
        format!(
            "{body}\n\n{footer_title} : {preferences_url}",
            body = body.trim_end(),
            footer_title = footer_title,
            preferences_url = preferences_url
        )
    });
    let html_body = template.html_body.map(|body| {
        format!(
            "{body}<p style=\"margin-top:24px;\"><a href=\"{url}\">{title}</a></p>",
            body = body.trim_end(),
            url = escape_html(&preferences_url),
            title = escape_html(footer_title)
        )
    });

    EmailTemplate {
        subject: template.subject,
        text_body,
        html_body,
    }
}

pub fn shared_trace_finalized_email(
    recipient_display_name: &str,
    owner_display_name: &str,
    journal_title: &str,
    journal_url: &str,
    interaction_date: NaiveDateTime,
    trace_content: &str,
) -> EmailTemplate {
    let excerpt = build_trace_excerpt(trace_content, 150);
    let subject = format!("{} a écrit dans son journal", owner_display_name);
    let interaction_date = interaction_date.format("%d/%m/%Y à %H:%M").to_string();
    let text_body = render_template(
        SHARED_TRACE_FINALIZED_TEXT,
        &[
            ("recipient_display_name", recipient_display_name.to_string()),
            ("owner_display_name", owner_display_name.to_string()),
            ("journal_title", journal_title.to_string()),
            ("journal_url", journal_url.to_string()),
            ("interaction_date", interaction_date.clone()),
            ("excerpt", excerpt.clone()),
        ],
    );
    let html_body = render_template(
        SHARED_TRACE_FINALIZED_HTML,
        &[
            (
                "recipient_display_name",
                escape_html(recipient_display_name),
            ),
            ("owner_display_name", escape_html(owner_display_name)),
            ("journal_title", escape_html(journal_title)),
            ("journal_url", escape_html(journal_url)),
            ("interaction_date", escape_html(&interaction_date)),
            ("excerpt_html", escape_html(&excerpt)),
        ],
    );

    append_contact_preferences_footer(EmailTemplate {
        subject,
        text_body: Some(text_body),
        html_body: Some(html_body),
    })
}

pub fn shared_journal_daily_digest_email(
    recipient_display_name: &str,
    digest_date_label: &str,
    items: Vec<SharedJournalDigestEmailItem>,
) -> EmailTemplate {
    let post_count = items.len();
    let owners = distinct_owner_display_names(&items);
    let owners_preview = if owners.len() > 2 {
        format!("{}, {}...", owners[0], owners[1])
    } else {
        join_owner_display_names(&owners)
    };
    let owners_sentence = join_owner_display_names(&owners);
    let subject = if post_count == 1 {
        format!(
            "Récap : 1 nouveau post le {} ({})",
            digest_date_label, owners_preview
        )
    } else {
        format!(
            "Récap : {} nouveaux posts le {} ({})",
            post_count, digest_date_label, owners_preview
        )
    };
    let intro_line = if owners.len() == 1 {
        format!("{} a écrit dans son journal.", owners_sentence)
    } else {
        format!(
            "{} ont écrit dans leur journal.",
            owners_sentence
        )
    };
    let summary_line = format!(
        "Voici un récapitulatif des traces écrites par vos amis le {}. Bonne lecture !",
        digest_date_label
    );
    let items_text = items
        .iter()
        .map(|item| {
            format!(
                "- {} • {} • {}\n{}\n{}\n",
                item.owner_display_name,
                item.journal_title,
                item.publishing_date_label,
                item.excerpt,
                item.journal_url
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let items_html = items
        .iter()
        .map(|item| {
            format!(
                "<div style=\"margin-bottom:16px;padding:16px 18px;background:#262626;border:1px solid #343434;border-radius:18px;\">
                    <div style=\"margin-bottom:6px;font-size:15px;color:#fff3e1;font-weight:600;\">{owner} • <a href=\"{url}\" style=\"color:#f4efe7;text-decoration:underline;\">{journal}</a></div>
                    <div style=\"margin-bottom:10px;font-size:13px;color:#b7ae9f;\">{date}</div>
                    <div style=\"font-size:16px;line-height:1.65;color:#f1ebdf;\">{excerpt}</div>
                </div>",
                owner = escape_html(&item.owner_display_name),
                url = escape_html(&item.journal_url),
                journal = escape_html(&item.journal_title),
                date = escape_html(&item.publishing_date_label),
                excerpt = escape_html(&item.excerpt),
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let home_url = format!(
        "{}/me/home",
        environment::get_app_base_url().trim_end_matches('/')
    );

    let text_body = render_template(
        SHARED_JOURNAL_DAILY_DIGEST_TEXT,
        &[
            ("recipient_display_name", recipient_display_name.to_string()),
            ("digest_date_label", digest_date_label.to_string()),
            ("intro_line", intro_line.clone()),
            ("summary_line", summary_line.clone()),
            ("items_text", items_text),
            ("home_url", home_url.clone()),
        ],
    );
    let html_body = render_template(
        SHARED_JOURNAL_DAILY_DIGEST_HTML,
        &[
            (
                "recipient_display_name",
                escape_html(recipient_display_name),
            ),
            ("digest_date_label", escape_html(digest_date_label)),
            ("intro_line", escape_html(&intro_line)),
            ("summary_line", escape_html(&summary_line)),
            ("items_html", items_html),
            ("home_url", escape_html(&home_url)),
        ],
    );

    append_contact_preferences_footer(EmailTemplate {
        subject,
        text_body: Some(text_body),
        html_body: Some(html_body),
    })
}

pub fn message_received_email(
    recipient_display_name: &str,
    sender_display_name: &str,
    message_content: &str,
    conversation_url: Option<&str>,
) -> EmailTemplate {
    let excerpt = build_trace_excerpt(message_content, 180);
    let subject = format!("{} vous a envoyé un message", sender_display_name);
    let text_body = render_template(
        MESSAGE_RECEIVED_TEXT,
        &[
            ("recipient_display_name", recipient_display_name.to_string()),
            ("sender_display_name", sender_display_name.to_string()),
            ("excerpt", excerpt.clone()),
            (
                "conversation_cta",
                conversation_url
                    .map(|url| format!("Ouvrir la conversation : {}", url))
                    .unwrap_or_else(|| {
                        "Connectez-vous à Matière Grise pour lire et répondre au message."
                            .to_string()
                    }),
            ),
        ],
    );
    let html_body = render_template(
        MESSAGE_RECEIVED_HTML,
        &[
            (
                "recipient_display_name",
                escape_html(recipient_display_name),
            ),
            ("sender_display_name", escape_html(sender_display_name)),
            ("excerpt_html", escape_html(&excerpt)),
            (
                "conversation_link_html",
                conversation_url
                    .map(|url| {
                        format!(
                            "<a href=\"{url}\">Ouvrir la conversation</a>",
                            url = escape_html(url)
                        )
                    })
                    .unwrap_or_else(|| {
                        "Connectez-vous à Matière Grise pour lire et répondre au message."
                            .to_string()
                    }),
            ),
        ],
    );

    append_contact_preferences_footer(EmailTemplate {
        subject,
        text_body: Some(text_body),
        html_body: Some(html_body),
    })
}

pub fn follow_request_received_email(
    recipient_display_name: &str,
    requester_display_name: &str,
    requester_handle: &str,
) -> EmailTemplate {
    let subject = format!("{} souhaite vous suivre", requester_display_name);
    let text_body = render_template(
        FOLLOW_REQUEST_RECEIVED_TEXT,
        &[
            ("recipient_display_name", recipient_display_name.to_string()),
            ("requester_display_name", requester_display_name.to_string()),
            ("requester_handle", requester_handle.to_string()),
        ],
    );
    let html_body = render_template(
        FOLLOW_REQUEST_RECEIVED_HTML,
        &[
            (
                "recipient_display_name",
                escape_html(recipient_display_name),
            ),
            (
                "requester_display_name",
                escape_html(requester_display_name),
            ),
            ("requester_handle", escape_html(requester_handle)),
        ],
    );

    append_contact_preferences_footer(EmailTemplate {
        subject,
        text_body: Some(text_body),
        html_body: Some(html_body),
    })
}

pub fn password_reset_email(recipient_display_name: &str, reset_url: &str) -> EmailTemplate {
    let subject = "Réinitialisation de votre mot de passe Matière Grise".to_string();
    let text_body = render_template(
        PASSWORD_RESET_TEXT,
        &[
            ("recipient_display_name", recipient_display_name.to_string()),
            ("reset_url", reset_url.to_string()),
        ],
    );
    let html_body = render_template(
        PASSWORD_RESET_HTML,
        &[
            (
                "recipient_display_name",
                escape_html(recipient_display_name),
            ),
            ("reset_url", escape_html(reset_url)),
        ],
    );

    append_contact_preferences_footer(EmailTemplate {
        subject,
        text_body: Some(text_body),
        html_body: Some(html_body),
    })
}

pub fn new_user_signup_email(first_name: &str, last_name: &str, users_url: &str) -> EmailTemplate {
    let subject = format!("Nouvel utilisateur inscrit : {} {}", first_name, last_name);
    let text_body = render_template(
        NEW_USER_SIGNUP_TEXT,
        &[
            ("first_name", first_name.to_string()),
            ("last_name", last_name.to_string()),
            ("users_url", users_url.to_string()),
        ],
    );
    let html_body = render_template(
        NEW_USER_SIGNUP_HTML,
        &[
            ("first_name", escape_html(first_name)),
            ("last_name", escape_html(last_name)),
            ("users_url", escape_html(users_url)),
        ],
    );

    append_contact_preferences_footer(EmailTemplate {
        subject,
        text_body: Some(text_body),
        html_body: Some(html_body),
    })
}

pub fn journal_access_granted_email(
    recipient_display_name: &str,
    owner_display_name: &str,
    journal_title: &str,
    journal_url: &str,
) -> EmailTemplate {
    let subject = format!("{} vous a donné accès à son journal", owner_display_name);
    let text_body = render_template(
        JOURNAL_ACCESS_GRANTED_TEXT,
        &[
            ("recipient_display_name", recipient_display_name.to_string()),
            ("owner_display_name", owner_display_name.to_string()),
            ("journal_title", journal_title.to_string()),
            ("journal_url", journal_url.to_string()),
        ],
    );
    let html_body = render_template(
        JOURNAL_ACCESS_GRANTED_HTML,
        &[
            (
                "recipient_display_name",
                escape_html(recipient_display_name),
            ),
            ("owner_display_name", escape_html(owner_display_name)),
            ("journal_title", escape_html(journal_title)),
            ("journal_url", escape_html(journal_url)),
        ],
    );

    append_contact_preferences_footer(EmailTemplate {
        subject,
        text_body: Some(text_body),
        html_body: Some(html_body),
    })
}

pub fn daily_recap_email(
    recipient_display_name: &str,
    mentor_display_name: &str,
    feedback_title: &str,
    feedback_preview: &str,
    recap_title: &str,
    recap_preview: &str,
    recap_url: &str,
) -> EmailTemplate {
    let subject = if feedback_title.trim().is_empty() {
        format!("🫀 Votre retour du jour de {}", mentor_display_name)
    } else {
        format!(
            "🫀 {} vous a laissé un retour : {}",
            mentor_display_name, feedback_title
        )
    };
    let text_body = render_template(
        DAILY_RECAP_FEEDBACK_TEXT,
        &[
            ("recipient_display_name", recipient_display_name.to_string()),
            ("mentor_display_name", mentor_display_name.to_string()),
            ("feedback_preview", feedback_preview.to_string()),
            ("recap_title", recap_title.to_string()),
            ("recap_preview", recap_preview.to_string()),
            ("recap_url", recap_url.to_string()),
        ],
    );
    let html_body = render_template(
        DAILY_RECAP_FEEDBACK_HTML,
        &[
            (
                "recipient_display_name",
                escape_html(recipient_display_name),
            ),
            ("mentor_display_name", escape_html(mentor_display_name)),
            ("feedback_preview_html", escape_html(feedback_preview)),
            ("recap_title", escape_html(recap_title)),
            ("recap_preview_html", escape_html(recap_preview)),
            ("recap_url", escape_html(recap_url)),
        ],
    );

    append_contact_preferences_footer(EmailTemplate {
        subject,
        text_body: Some(text_body),
        html_body: Some(html_body),
    })
}
