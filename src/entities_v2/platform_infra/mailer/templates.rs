use chrono::NaiveDateTime;

#[derive(Debug, Clone)]
pub struct EmailTemplate {
    pub subject: String,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
}

const SHARED_TRACE_FINALIZED_TEXT: &str = include_str!("templates/shared_trace_finalized.txt");
const SHARED_TRACE_FINALIZED_HTML: &str = include_str!("templates/shared_trace_finalized.html");

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
            ("interaction_date", interaction_date.clone()),
            ("excerpt", excerpt.clone()),
        ],
    );
    let html_body = render_template(
        SHARED_TRACE_FINALIZED_HTML,
        &[
            ("recipient_display_name", escape_html(recipient_display_name)),
            ("owner_display_name", escape_html(owner_display_name)),
            ("journal_title", escape_html(journal_title)),
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
