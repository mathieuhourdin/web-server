use chrono::NaiveDateTime;

use crate::environment;

#[derive(Debug, Clone)]
pub struct EmailTemplate {
    pub subject: String,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
}

const SHARED_TRACE_FINALIZED_TEXT: &str = include_str!("templates/shared_trace_finalized.txt");
const EMAIL_LAYOUT_HTML: &str = include_str!("templates/layout.html");
const SHARED_TRACE_FINALIZED_HTML: &str = include_str!("templates/shared_trace_finalized.html");
const TRACE_MENTION_TEXT: &str = include_str!("templates/trace_mention.txt");
const TRACE_MENTION_HTML: &str = include_str!("templates/trace_mention.html");
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
const WEEKLY_RECAP_FEEDBACK_TEXT: &str = include_str!("templates/weekly_recap_feedback.txt");
const WEEKLY_RECAP_FEEDBACK_HTML: &str = include_str!("templates/weekly_recap_feedback.html");
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
    // Substitute only placeholders from the template, never placeholder-like
    // content inside a user's name or excerpt inserted by an earlier replacement.
    let mut rendered = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        rendered.push_str(&rest[..start]);
        let Some(end) = rest[start + 2..].find("}}") else {
            rendered.push_str(&rest[start..]);
            return rendered;
        };
        let key = &rest[start + 2..start + 2 + end];
        if let Some((_, value)) = variables.iter().find(|(name, _)| *name == key) {
            rendered.push_str(value);
        } else {
            rendered.push_str(&rest[start..start + end + 4]);
        }
        rest = &rest[start + end + 4..];
    }
    rendered.push_str(rest);
    rendered
}

fn app_icon_url() -> String {
    format!(
        "{}/public/assets/email-icon.png",
        environment::get_api_url().trim_end_matches('/')
    )
}

// Inline the cream defaults for clients that strip <style>; classes remain for
// progressive dark-mode overrides. Mobile palette: AppColors.light / .dark.
fn render_email_layout(
    subject: &str,
    body_html: &str,
    preferences_url: &str,
    icon_url: &str,
) -> String {
    let mut html = render_template(
        EMAIL_LAYOUT_HTML,
        &[
            ("email_subject", escape_html(subject)),
            ("body_html", body_html.to_owned()),
            ("preferences_url", escape_html(preferences_url)),
            ("app_icon_url", escape_html(icon_url)),
        ],
    );
    let styles = [
        ("email-greeting", "margin:0 0 12px;color:#5B5648;font-size:15px;line-height:1.6;"),
        ("email-title", "margin:0 0 22px;color:#211F1A;font-family:Georgia,'Times New Roman',serif;font-size:26px;font-weight:400;line-height:1.3;letter-spacing:-0.4px;"),
        ("email-copy", "margin:0 0 20px;color:#211F1A;font-size:17px;line-height:1.7;"),
        ("email-meta", "margin:0 0 18px;color:#5B5648;font-family:Arial,Helvetica,sans-serif;font-size:13px;line-height:1.7;"),
        ("email-kicker", "margin:0 0 10px;color:#5B5648;font-family:Arial,Helvetica,sans-serif;font-size:11px;line-height:1.6;font-weight:600;letter-spacing:0.08em;text-transform:uppercase;"),
        ("email-section", "margin:28px 0 12px;color:#211F1A;font-size:19px;font-weight:400;line-height:1.4;"),
        ("email-excerpt", "margin:0 0 24px;padding:16px 18px;background-color:#ECE6D9;border-left:2px solid #C9BEAA;color:#211F1A;font-size:17px;line-height:1.75;word-wrap:break-word;"),
        ("email-link", "color:#08134F;text-decoration:underline;"),
        ("email-action", "margin:24px 0 8px;border-collapse:separate;"),
        ("email-button-cell", "background-color:#08134F;border-radius:8px;mso-padding-alt:14px 20px;text-align:center;"),
        ("email-button", "display:inline-block;padding:14px 20px;border:1px solid #08134F;border-radius:8px;background-color:#08134F;color:#FBF9F3;font-family:Arial,Helvetica,sans-serif;font-size:14px;font-weight:600;line-height:1.4;text-align:center;text-decoration:none;"),
        ("email-small", "margin:20px 0 0;color:#5B5648;font-family:Arial,Helvetica,sans-serif;font-size:13px;line-height:1.7;word-wrap:break-word;"),
        ("email-fallback", "margin:8px 0 0;font-family:Arial,Helvetica,sans-serif;font-size:12px;line-height:1.7;word-break:break-all;"),
        ("email-digest-item", "margin:0 0 24px;padding:0 0 24px;border-bottom:1px solid #DDD8CE;"),
        ("email-item-title", "margin:0 0 8px;color:#211F1A;font-size:19px;font-weight:400;line-height:1.5;"),
    ];
    for (class, style) in styles {
        html = html.replace(
            &format!("class=\"{class}\""),
            &format!("class=\"{class}\" style=\"{style}\""),
        );
    }
    html
}

fn escape_html_multiline(value: &str) -> String {
    escape_html(value)
        .replace("\r\n", "\n")
        .replace('\n', "<br>\n")
}

fn app_url(path: &str) -> String {
    format!(
        "{}/{}",
        environment::get_app_base_url().trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

/// Canonical web/mobile link; opening it still requires the recipient's access.
pub(crate) fn trace_url(trace_id: uuid::Uuid) -> String {
    trace_url_with_base(&environment::get_app_base_url(), trace_id)
}

fn trace_url_with_base(base_url: &str, trace_id: uuid::Uuid) -> String {
    format!("{}/traces/{trace_id}", base_url.trim_end_matches('/'))
}

#[cfg(test)]
mod trace_link_tests {
    #[test]
    fn trace_email_links_match_the_web_and_mobile_route() {
        let id = uuid::Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();
        for base in ["https://app.hupo.fr", "https://app.hupo.fr/"] {
            assert_eq!(
                super::trace_url_with_base(base, id),
                "https://app.hupo.fr/traces/00000000-0000-4000-8000-000000000001"
            );
        }
        assert_eq!(
            super::trace_url_with_base("http://localhost:5173/", id),
            "http://localhost:5173/traces/00000000-0000-4000-8000-000000000001"
        );
    }
}

fn append_contact_preferences_footer(template: EmailTemplate) -> EmailTemplate {
    let preferences_url = app_url("/me/user#email-preferences");
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
        render_email_layout(&template.subject, &body, &preferences_url, &app_icon_url())
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
            ("excerpt_html", escape_html_multiline(&excerpt)),
        ],
    );

    append_contact_preferences_footer(EmailTemplate {
        subject,
        text_body: Some(text_body),
        html_body: Some(html_body),
    })
}

pub fn trace_mention_email(
    recipient_display_name: &str,
    owner_display_name: &str,
    journal_title: &str,
    journal_url: &str,
    interaction_date: NaiveDateTime,
    trace_content: &str,
) -> EmailTemplate {
    let excerpt = build_trace_excerpt(trace_content, 150);
    let subject = format!("{} vous a mentionné dans une trace", owner_display_name);
    let interaction_date = interaction_date.format("%d/%m/%Y à %H:%M").to_string();
    let variables = [
        ("recipient_display_name", recipient_display_name.to_string()),
        ("owner_display_name", owner_display_name.to_string()),
        ("journal_title", journal_title.to_string()),
        ("journal_url", journal_url.to_string()),
        ("interaction_date", interaction_date.clone()),
        ("excerpt", excerpt.clone()),
    ];
    let html_variables = [
        (
            "recipient_display_name",
            escape_html(recipient_display_name),
        ),
        ("owner_display_name", escape_html(owner_display_name)),
        ("journal_title", escape_html(journal_title)),
        ("journal_url", escape_html(journal_url)),
        ("interaction_date", escape_html(&interaction_date)),
        ("excerpt_html", escape_html_multiline(&excerpt)),
    ];

    append_contact_preferences_footer(EmailTemplate {
        subject,
        text_body: Some(render_template(TRACE_MENTION_TEXT, &variables)),
        html_body: Some(render_template(TRACE_MENTION_HTML, &html_variables)),
    })
}

pub fn shared_journal_weekly_digest_email(
    recipient_display_name: &str,
    period_label: &str,
    items: Vec<SharedJournalDigestEmailItem>,
) -> EmailTemplate {
    shared_journal_digest_email(recipient_display_name, period_label, items, true)
}

pub fn shared_journal_daily_digest_email(
    recipient_display_name: &str,
    digest_date_label: &str,
    items: Vec<SharedJournalDigestEmailItem>,
) -> EmailTemplate {
    shared_journal_digest_email(recipient_display_name, digest_date_label, items, false)
}

fn shared_journal_digest_email(
    recipient_display_name: &str,
    digest_date_label: &str,
    items: Vec<SharedJournalDigestEmailItem>,
    weekly: bool,
) -> EmailTemplate {
    let owners = distinct_owner_display_names(&items);
    let owners_preview = if owners.len() > 2 {
        format!("{}, {}...", owners[0], owners[1])
    } else {
        join_owner_display_names(&owners)
    };
    let owners_sentence = join_owner_display_names(&owners);
    let subject = if weekly {
        "Votre semaine de lectures sur hupo".to_string()
    } else if owners.len() == 1 {
        format!("{} a écrit dans son journal", owners_preview)
    } else {
        format!("{} ont écrit dans leur journal", owners_preview)
    };
    let intro_line = if owners.len() == 1 {
        format!("{} a écrit dans son journal.", owners_sentence)
    } else {
        format!("{} ont écrit dans leur journal.", owners_sentence)
    };
    let summary_line = if weekly {
        format!(
            "Voici les traces de vos amis que vous n’avez pas encore lues, {}. Bonne lecture !",
            digest_date_label
        )
    } else {
        format!(
            "Voici un récapitulatif des traces écrites par vos amis le {}. Bonne lecture !",
            digest_date_label
        )
    };
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
                "<div class=\"email-digest-item\">
                    <h2 class=\"email-item-title\">{owner} · <a class=\"email-link\" href=\"{url}\">{journal}</a></h2>
                    <p class=\"email-meta\">{date}</p>
                    <p class=\"email-copy\">{excerpt}</p>
                </div>",
                owner = escape_html(&item.owner_display_name),
                url = escape_html(&item.journal_url),
                journal = escape_html(&item.journal_title),
                date = escape_html(&item.publishing_date_label),
                excerpt = escape_html_multiline(&item.excerpt),
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
    let fallback_conversation_url = app_url("/me/conversation");
    let conversation_url = conversation_url.unwrap_or(&fallback_conversation_url);
    let text_body = render_template(
        MESSAGE_RECEIVED_TEXT,
        &[
            ("recipient_display_name", recipient_display_name.to_string()),
            ("sender_display_name", sender_display_name.to_string()),
            ("excerpt", excerpt.clone()),
            (
                "conversation_cta",
                format!("Ouvrir la conversation : {}", conversation_url),
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
            ("excerpt_html", escape_html_multiline(&excerpt)),
            (
                "conversation_link_html",
                format!(
                    "<a class=\"email-button\" href=\"{url}\">Ouvrir la conversation</a>",
                    url = escape_html(conversation_url)
                ),
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
    let followers_url = app_url("/social/followers");
    let text_body = render_template(
        FOLLOW_REQUEST_RECEIVED_TEXT,
        &[
            ("recipient_display_name", recipient_display_name.to_string()),
            ("requester_display_name", requester_display_name.to_string()),
            ("requester_handle", requester_handle.to_string()),
            ("followers_url", followers_url.clone()),
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
            ("followers_url", escape_html(&followers_url)),
        ],
    );

    append_contact_preferences_footer(EmailTemplate {
        subject,
        text_body: Some(text_body),
        html_body: Some(html_body),
    })
}

pub fn password_reset_email(recipient_display_name: &str, reset_url: &str) -> EmailTemplate {
    let subject = "Réinitialisation de votre mot de passe hupo".to_string();
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
        format!("Votre retour du jour de {}", mentor_display_name)
    } else {
        format!(
            "{} vous a laissé un retour : {}",
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
            (
                "feedback_preview_html",
                escape_html_multiline(feedback_preview),
            ),
            ("recap_title", escape_html(recap_title)),
            ("recap_preview_html", escape_html_multiline(recap_preview)),
            ("recap_url", escape_html(recap_url)),
        ],
    );

    append_contact_preferences_footer(EmailTemplate {
        subject,
        text_body: Some(text_body),
        html_body: Some(html_body),
    })
}

pub fn weekly_recap_email(
    recipient_display_name: &str,
    mentor_display_name: &str,
    feedback_title: &str,
    feedback_preview: &str,
    recap_title: &str,
    recap_preview: &str,
    recap_url: &str,
) -> EmailTemplate {
    let subject = if feedback_title.trim().is_empty() {
        format!("Votre retour de la semaine de {}", mentor_display_name)
    } else {
        format!(
            "{} vous a laissé un retour : {}",
            mentor_display_name, feedback_title
        )
    };
    let text_body = render_template(
        WEEKLY_RECAP_FEEDBACK_TEXT,
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
        WEEKLY_RECAP_FEEDBACK_HTML,
        &[
            (
                "recipient_display_name",
                escape_html(recipient_display_name),
            ),
            ("mentor_display_name", escape_html(mentor_display_name)),
            (
                "feedback_preview_html",
                escape_html_multiline(feedback_preview),
            ),
            ("recap_title", escape_html(recap_title)),
            ("recap_preview_html", escape_html_multiline(recap_preview)),
            ("recap_url", escape_html(recap_url)),
        ],
    );

    append_contact_preferences_footer(EmailTemplate {
        subject,
        text_body: Some(text_body),
        html_body: Some(html_body),
    })
}

#[cfg(test)]
mod email_design_tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn renders_all_email_variants_with_mobile_cream_defaults() {
        let url = "https://app.hupo.fr/traces/00000000-0000-4000-8000-000000000001";
        let variables = [
            ("recipient_display_name",escape_html("Camille")),
            ("owner_display_name",escape_html("Lucie")),
            ("sender_display_name",escape_html("Lucie")),
            ("requester_display_name",escape_html("Lucie")),
            ("requester_handle",escape_html("@lucie")),
            ("first_name",escape_html("Camille")),
            ("last_name",escape_html("Martin")),
            ("journal_title",escape_html("Au fil des jours")),
            ("journal_url",escape_html(url)),
            ("interaction_date",escape_html("2 novembre 2026, à 18 h 30")),
            ("excerpt_html",escape_html_multiline("Ce matin, j’ai repris le chemin qui longe la rivière.\n\nJe crois que j’avais surtout besoin de laisser un peu de place à ce qui vient.")),
            ("conversation_link_html",format!("<a class=\"email-button\" href=\"{url}\">Ouvrir la conversation</a>")),
            ("followers_url","https://app.hupo.fr/social/followers".into()),
            ("users_url","https://app.hupo.fr/admin/users".into()),
            ("reset_url",escape_html("https://app.hupo.fr/reset-password?token=synthetic-demo-only&email=camille%40example.invalid")),
            ("mentor_display_name",escape_html("Ariane")),
            ("feedback_preview_html",escape_html_multiline("Il y a dans ces quelques lignes une attention aux petites choses qui revient souvent dans vos écrits.\n\nQu’aimeriez-vous en garder pour les jours qui viennent ?")),
            ("recap_title",escape_html("Retrouver un peu d’espace")),
            ("recap_preview_html",escape_html("Une promenade, une conversation et le désir de retrouver un rythme qui vous ressemble.")),
            ("recap_url",escape_html(url)),
            ("digest_date_label",escape_html("du 26 octobre au 1er novembre")),
            ("intro_line",escape_html("Lucie et Noé ont écrit dans leurs journaux.")),
            ("summary_line",escape_html("Quelques traces de vos proches vous attendent, à lire quand vous en aurez envie.")),
            ("items_html",format!("<div class=\"email-digest-item\"><h2 class=\"email-item-title\">Lucie · <a class=\"email-link\" href=\"{url}\">Au fil des jours</a></h2><p class=\"email-meta\">1er novembre · 18 h 30</p><p class=\"email-copy\">Un dimanche sans programme. Juste le temps de regarder la lumière changer.</p></div><div class=\"email-digest-item\"><h2 class=\"email-item-title\">Noé · <a class=\"email-link\" href=\"{url}\">En chemin</a></h2><p class=\"email-meta\">31 octobre · 10 h 15</p><p class=\"email-copy\">J’ai commencé à écrire une histoire. Pour l’instant, je ne sais pas encore où elle me mène.</p></div>")),
            ("home_url","https://app.hupo.fr/me/home".into()),
        ];
        let icon = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD
                .encode(include_bytes!("../../../../public/assets/email-icon.png"))
        );
        let templates = [
            ("message_received", MESSAGE_RECEIVED_HTML),
            ("password_reset", PASSWORD_RESET_HTML),
            ("shared_trace", SHARED_TRACE_FINALIZED_HTML),
            ("mention", TRACE_MENTION_HTML),
            ("follow_request", FOLLOW_REQUEST_RECEIVED_HTML),
            ("journal_access", JOURNAL_ACCESS_GRANTED_HTML),
            ("new_user", NEW_USER_SIGNUP_HTML),
            ("digest", SHARED_JOURNAL_DAILY_DIGEST_HTML),
            ("daily_feedback", DAILY_RECAP_FEEDBACK_HTML),
            ("weekly_feedback", WEEKLY_RECAP_FEEDBACK_HTML),
        ];
        for (name, fragment) in templates {
            let html = render_email_layout(
                "Des nouvelles sur hupo",
                &render_template(fragment, &variables),
                "https://app.hupo.fr/me/user#email-preferences",
                &icon,
            );
            assert!(html.starts_with("<!doctype html>"), "{name}");
            assert_eq!(html.matches("<!doctype html>").count(), 1, "{name}");
            assert!(html.contains("background-color:#F1ECE2"), "{name}");
            assert!(html.contains("class=\"email-button\" style=\""), "{name}");
            assert!(html.contains("prefers-color-scheme: dark"), "{name}");
            assert!(
                html.find("Gérer mes préférences email").unwrap() < html.find("</body>").unwrap(),
                "{name}"
            );
            assert!(!html.contains("{{"), "Unresolved placeholder in {name}");
            assert!(!html.contains("box-shadow"), "{name}");
            if let Ok(directory) = std::env::var("HUPO_EMAIL_PREVIEW_DIR") {
                let path = std::path::Path::new(&directory);
                assert!(path.is_dir(), "Create a private preview directory first");
                std::fs::write(path.join(format!("{name}.html")), html).unwrap();
            }
        }
    }

    #[test]
    fn rendering_keeps_user_content_literal_and_escapes_layout_fields() {
        let name = "Camille {{body_html}} <script>alert(1)</script>";
        let body = render_template(
            "<p class=\"email-copy\">{{name}}</p>",
            &[("name", escape_html(name))],
        );
        let html = render_email_layout(
            name,
            &body,
            "https://app.hupo.fr/settings?a=1&b=2",
            "https://api.hupo.fr/public/assets/email-icon.png",
        );
        assert!(html.contains("Camille {{body_html}} &lt;script&gt;"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("settings?a=1&amp;b=2"));
        assert_eq!(html.matches("<p class=\"email-copy\"").count(), 1);
        assert_eq!(
            escape_html_multiline("<b>A</b>\r\nB"),
            "&lt;b&gt;A&lt;/b&gt;<br>\nB"
        );
    }
}
