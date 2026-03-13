use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::environment;
use crate::work_analyzer::observability::format_text_log_field;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendEmailRequest {
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub text: Option<String>,
    pub html: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentEmail {
    pub id: String,
}

#[derive(Debug, Serialize)]
struct ResendSendEmailRequest<'a> {
    from: &'a str,
    to: &'a [String],
    subject: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    html: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct ResendSendEmailResponse {
    id: String,
}

pub async fn send_email(request: SendEmailRequest) -> Result<SentEmail, PpdcError> {
    if request.to.is_empty() {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Email requires at least one recipient".to_string(),
        ));
    }
    if request.text.is_none() && request.html.is_none() {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Email requires at least one body: text or html".to_string(),
        ));
    }

    let client = Client::new();
    let api_key = environment::get_resend_api_key();
    let base_url = environment::get_resend_api_base_url();

    let payload = ResendSendEmailRequest {
        from: &request.from,
        to: &request.to,
        subject: &request.subject,
        text: request.text.as_deref(),
        html: request.html.as_deref(),
    };

    tracing::info!(
        target: "mailer",
        "resend_send_email_start recipients_count={} {} {} {} {}",
        request.to.len(),
        format_text_log_field("from", &request.from),
        format_text_log_field("subject", &request.subject),
        request
            .text
            .as_ref()
            .map(|body| format_text_log_field("text", body))
            .unwrap_or_else(|| "text_len=0".to_string()),
        request
            .html
            .as_ref()
            .map(|body| format_text_log_field("html", body))
            .unwrap_or_else(|| "html_len=0".to_string())
    );

    let response = client
        .post(format!("{}/emails", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await
        .map_err(|error| {
            PpdcError::new(
                502,
                ErrorType::InternalError,
                format!("Failed to send request to Resend: {}", error),
            )
        })?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(PpdcError::new(
            status.as_u16() as u32,
            ErrorType::ApiError,
            format!("Resend API error ({}): {}", status, error_body),
        ));
    }

    let response_body = response.json::<ResendSendEmailResponse>().await.map_err(|error| {
        PpdcError::new(
            502,
            ErrorType::InternalError,
            format!("Failed to decode Resend response: {}", error),
        )
    })?;

    tracing::info!(
        target: "mailer",
        "resend_send_email_success email_id={}",
        response_body.id
    );

    Ok(SentEmail {
        id: response_body.id,
    })
}
