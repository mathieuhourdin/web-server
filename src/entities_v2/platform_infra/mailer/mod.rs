mod client;
mod outbound_email;
mod templates;

pub use client::{send_email, SendEmailRequest, SentEmail};
pub use outbound_email::{
    process_pending_email, process_pending_emails, NewOutboundEmail, OutboundEmail,
    OutboundEmailProvider, OutboundEmailStatus,
};
pub use templates::{
    follow_request_received_email, message_received_email, password_reset_email,
    shared_trace_finalized_email, EmailTemplate,
};
