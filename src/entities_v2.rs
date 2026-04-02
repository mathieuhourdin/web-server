pub mod analysis_orchestration;
pub mod derived_context;
pub mod platform_infra;
pub mod records;
pub mod shared;
pub mod social;

// Backward-compatible re-exports for existing imports across the codebase.
pub use analysis_orchestration::{landscape_analysis, lens};
pub use derived_context::{analysis_summary, element, landmark, reference, trace_mirror};
pub use platform_infra::{error, llm_call, session, transcription, usage_event, user, user_secure_action};
pub use records::{journal, journal_import, trace};
pub use shared::MaturingState;
pub use social::{journal_grant, message, post, post_grant, relationship, user_post_state};
