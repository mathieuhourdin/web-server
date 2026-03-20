use chrono::NaiveDateTime;

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

fn build_trace_excerpt(content: &str, max_chars: usize) -> String {
    let excerpt = content.trim().chars().take(max_chars).collect::<String>();
    if content.trim().chars().count() > max_chars {
        format!("{}...", excerpt)
    } else {
        excerpt
    }
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
    let interaction_date = interaction_date.to_string();
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

    EmailTemplate {
        subject,
        text_body: Some(text_body),
        html_body: Some(html_body),
    }
}

pub fn message_received_email(
    recipient_display_name: &str,
    sender_display_name: &str,
    message_title: &str,
    message_content: &str,
    conversation_url: Option<&str>,
) -> EmailTemplate {
    let excerpt = build_trace_excerpt(message_content, 180);
    let subject = if message_title.trim().is_empty() {
        format!("{} vous a envoye un message", sender_display_name)
    } else {
        format!(
            "{} vous a envoye un message : {}",
            sender_display_name, message_title
        )
    };
    let text_body = render_template(
        MESSAGE_RECEIVED_TEXT,
        &[
            ("recipient_display_name", recipient_display_name.to_string()),
            ("sender_display_name", sender_display_name.to_string()),
            ("message_title", message_title.to_string()),
            ("excerpt", excerpt.clone()),
            (
                "conversation_cta",
                conversation_url
                    .map(|url| format!("Ouvrir la conversation : {}", url))
                    .unwrap_or_else(|| {
                        "Connectez-vous a Matiere Grise pour lire et repondre au message."
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
            ("message_title", escape_html(message_title)),
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
                        "Connectez-vous a Matiere Grise pour lire et repondre au message."
                            .to_string()
                    }),
            ),
        ],
    );

    EmailTemplate {
        subject,
        text_body: Some(text_body),
        html_body: Some(html_body),
    }
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

    EmailTemplate {
        subject,
        text_body: Some(text_body),
        html_body: Some(html_body),
    }
}

pub fn password_reset_email(recipient_display_name: &str, reset_url: &str) -> EmailTemplate {
    let subject = "Reinitialisation de votre mot de passe Matiere Grise".to_string();
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

    EmailTemplate {
        subject,
        text_body: Some(text_body),
        html_body: Some(html_body),
    }
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

    EmailTemplate {
        subject,
        text_body: Some(text_body),
        html_body: Some(html_body),
    }
}
