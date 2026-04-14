mod client;
mod outbound_email;
mod templates;

pub use client::{send_email, SendEmailRequest, SentEmail};
pub use outbound_email::{
    process_pending_email, process_pending_emails, NewOutboundEmail, OutboundEmail,
    OutboundEmailProvider, OutboundEmailStatus,
};
pub use templates::{
    daily_recap_email, follow_request_received_email, journal_access_granted_email,
    message_received_email, new_user_signup_email, password_reset_email,
    shared_trace_finalized_email, EmailTemplate,
};
