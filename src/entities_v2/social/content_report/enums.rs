use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentReportReason {
    Aggressive,
    Harassment,
    Hate,
    Sexual,
    Spam,
    SelfHarm,
    Misinformation,
    Privacy,
    Other,
}

impl ContentReportReason {
    pub fn to_db(self) -> &'static str {
        match self {
            ContentReportReason::Aggressive => "AGGRESSIVE",
            ContentReportReason::Harassment => "HARASSMENT",
            ContentReportReason::Hate => "HATE",
            ContentReportReason::Sexual => "SEXUAL",
            ContentReportReason::Spam => "SPAM",
            ContentReportReason::SelfHarm => "SELF_HARM",
            ContentReportReason::Misinformation => "MISINFORMATION",
            ContentReportReason::Privacy => "PRIVACY",
            ContentReportReason::Other => "OTHER",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "AGGRESSIVE" | "aggressive" => ContentReportReason::Aggressive,
            "HARASSMENT" | "harassment" => ContentReportReason::Harassment,
            "HATE" | "hate" => ContentReportReason::Hate,
            "SEXUAL" | "sexual" => ContentReportReason::Sexual,
            "SPAM" | "spam" => ContentReportReason::Spam,
            "SELF_HARM" | "self_harm" => ContentReportReason::SelfHarm,
            "MISINFORMATION" | "misinformation" => ContentReportReason::Misinformation,
            "PRIVACY" | "privacy" => ContentReportReason::Privacy,
            _ => ContentReportReason::Other,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentReportStatus {
    Open,
    Reviewed,
    Dismissed,
    ActionTaken,
}

impl ContentReportStatus {
    pub fn to_db(self) -> &'static str {
        match self {
            ContentReportStatus::Open => "OPEN",
            ContentReportStatus::Reviewed => "REVIEWED",
            ContentReportStatus::Dismissed => "DISMISSED",
            ContentReportStatus::ActionTaken => "ACTION_TAKEN",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "REVIEWED" | "reviewed" => ContentReportStatus::Reviewed,
            "DISMISSED" | "dismissed" => ContentReportStatus::Dismissed,
            "ACTION_TAKEN" | "action_taken" => ContentReportStatus::ActionTaken,
            _ => ContentReportStatus::Open,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentReportSourceKind {
    Message,
    Trace,
    Document,
    Album,
}

impl ContentReportSourceKind {
    pub fn to_db(self) -> &'static str {
        match self {
            ContentReportSourceKind::Message => "MESSAGE",
            ContentReportSourceKind::Trace => "TRACE",
            ContentReportSourceKind::Document => "DOCUMENT",
            ContentReportSourceKind::Album => "ALBUM",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "MESSAGE" | "message" => ContentReportSourceKind::Message,
            "TRACE" | "trace" => ContentReportSourceKind::Trace,
            "DOCUMENT" | "document" => ContentReportSourceKind::Document,
            "ALBUM" | "album" => ContentReportSourceKind::Album,
            _ => ContentReportSourceKind::Message,
        }
    }
}
