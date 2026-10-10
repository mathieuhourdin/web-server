use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use super::enums::{AnalysisSummaryType, SummaryEmojiKind};

/// A display cue, not an emotional diagnosis. Keep malformed model output from
/// failing an otherwise valid recap, including composed emoji sequences.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct SummaryEmojis {
    pub activity_emoji: Option<String>,
    pub mood_emoji: Option<String>,
    pub dominant_emoji_kind: Option<SummaryEmojiKind>,
}

impl SummaryEmojis {
    pub fn normalized(mut self) -> Self {
        self.activity_emoji = normalize_emoji(self.activity_emoji);
        self.mood_emoji = normalize_emoji(self.mood_emoji);
        self.dominant_emoji_kind = match self.dominant_emoji_kind {
            Some(SummaryEmojiKind::Activity) if self.activity_emoji.is_some() => {
                self.dominant_emoji_kind
            }
            Some(SummaryEmojiKind::Mood) if self.mood_emoji.is_some() => self.dominant_emoji_kind,
            _ if self.activity_emoji.is_some() => Some(SummaryEmojiKind::Activity),
            _ if self.mood_emoji.is_some() => Some(SummaryEmojiKind::Mood),
            _ => None,
        };
        self
    }

    pub fn display_emoji(&self) -> Option<String> {
        match self.dominant_emoji_kind {
            Some(SummaryEmojiKind::Activity) => self.activity_emoji.clone(),
            Some(SummaryEmojiKind::Mood) => self.mood_emoji.clone(),
            None => None,
        }
    }
}

fn normalize_emoji(value: Option<String>) -> Option<String> {
    use std::sync::OnceLock;
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    let value = value?.trim().to_string();
    if value.len() > 128 {
        return None;
    }
    let pattern = PATTERN.get_or_init(|| regex::Regex::new(
        r"^(?:\p{Regional_Indicator}{2}|[0-9#*]\u{FE0F}?\u{20E3}|\p{Extended_Pictographic}\u{FE0F}?\p{Emoji_Modifier}?(?:\u{200D}\p{Extended_Pictographic}\u{FE0F}?\p{Emoji_Modifier}?)*(?:[\u{E0061}-\u{E007A}]+\u{E007F})?)$"
    ).expect("valid emoji sequence pattern"));
    pattern.is_match(&value).then_some(value)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MeaningfulEvent {
    pub title: String,
    pub description: String,
    pub event_date: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AnalysisSummary {
    pub id: Uuid,
    pub landscape_analysis_id: Uuid,
    pub user_id: Uuid,
    pub summary_type: AnalysisSummaryType,
    pub title: String,
    pub short_content: String,
    pub content: String,
    pub meaningful_event: Option<MeaningfulEvent>,
    pub activity_emoji: Option<String>,
    pub mood_emoji: Option<String>,
    pub dominant_emoji_kind: Option<SummaryEmojiKind>,
    /// Derived from the stored selector; never independently persisted.
    pub emoji: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewAnalysisSummaryDto {
    pub summary_type: Option<AnalysisSummaryType>,
    pub title: String,
    pub short_content: Option<String>,
    pub content: String,
    pub meaningful_event: Option<MeaningfulEvent>,
    #[serde(
        default,
        deserialize_with = "crate::serde_helpers::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub activity_emoji: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::serde_helpers::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub mood_emoji: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::serde_helpers::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub dominant_emoji_kind: Option<Option<SummaryEmojiKind>>,
}

#[derive(Debug, Clone)]
pub struct NewAnalysisSummary {
    pub landscape_analysis_id: Uuid,
    pub user_id: Uuid,
    pub summary_type: AnalysisSummaryType,
    pub title: String,
    pub short_content: String,
    pub content: String,
    pub meaningful_event: Option<MeaningfulEvent>,
    pub emojis: SummaryEmojis,
}

impl NewAnalysisSummary {
    pub fn new(payload: NewAnalysisSummaryDto, landscape_analysis_id: Uuid, user_id: Uuid) -> Self {
        Self {
            landscape_analysis_id,
            user_id,
            summary_type: payload
                .summary_type
                .unwrap_or(AnalysisSummaryType::PeriodRecap),
            title: payload.title,
            short_content: payload
                .short_content
                .unwrap_or_else(|| payload.content.clone()),
            content: payload.content,
            meaningful_event: payload.meaningful_event,
            emojis: SummaryEmojis {
                activity_emoji: payload.activity_emoji.flatten(),
                mood_emoji: payload.mood_emoji.flatten(),
                dominant_emoji_kind: payload.dominant_emoji_kind.flatten(),
            }
            .normalized(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_single_and_composed_emoji_cues() {
        for value in ["🌱", "😌", "❤️", "👩🏽‍🔬", "👨‍👩‍👧‍👦", "🇫🇷", "1️⃣"]
        {
            assert_eq!(
                normalize_emoji(Some(value.to_string())),
                Some(value.to_string())
            );
        }
        assert_eq!(normalize_emoji(Some("  🌱  ".into())), Some("🌱".into()));
        for value in ["", "happy", "1", "a", "🙂🙃", "🌱 😌", "<script>", "👩👩"] {
            assert_eq!(normalize_emoji(Some(value.to_string())), None, "{value}");
        }
    }

    #[test]
    fn dominant_choice_selects_an_available_cue() {
        let emojis = SummaryEmojis {
            activity_emoji: Some("🌱".into()),
            mood_emoji: Some("😌".into()),
            dominant_emoji_kind: Some(SummaryEmojiKind::Mood),
        }
        .normalized();
        assert_eq!(emojis.display_emoji().as_deref(), Some("😌"));
        let fallback = SummaryEmojis {
            mood_emoji: None,
            ..emojis
        }
        .normalized();
        assert_eq!(
            fallback.dominant_emoji_kind,
            Some(SummaryEmojiKind::Activity)
        );
        assert_eq!(fallback.display_emoji().as_deref(), Some("🌱"));
        let missing = SummaryEmojis {
            activity_emoji: None,
            mood_emoji: None,
            dominant_emoji_kind: Some(SummaryEmojiKind::Mood),
        }
        .normalized();
        assert_eq!(missing.dominant_emoji_kind, None);
        assert_eq!(missing.display_emoji(), None);
    }

    #[test]
    fn malformed_cues_do_not_fail_the_recap() {
        let emojis = SummaryEmojis {
            activity_emoji: Some("gardening".into()),
            mood_emoji: Some("😌".into()),
            dominant_emoji_kind: Some(SummaryEmojiKind::Activity),
        }
        .normalized();
        assert_eq!(emojis.activity_emoji, None);
        assert_eq!(emojis.dominant_emoji_kind, Some(SummaryEmojiKind::Mood));
        assert_eq!(emojis.display_emoji().as_deref(), Some("😌"));
    }

    #[test]
    fn summary_write_dto_distinguishes_missing_null_and_supplied_cues() {
        let base = serde_json::json!({"title":"Day", "content":"Recap"});
        let missing: NewAnalysisSummaryDto = serde_json::from_value(base.clone()).unwrap();
        assert_eq!(missing.activity_emoji, None);
        assert_eq!(missing.mood_emoji, None);
        assert_eq!(missing.dominant_emoji_kind, None);
        let mut explicit = base;
        explicit["activity_emoji"] = serde_json::Value::Null;
        explicit["mood_emoji"] = serde_json::json!("😌");
        explicit["dominant_emoji_kind"] = serde_json::json!("mood");
        let supplied: NewAnalysisSummaryDto = serde_json::from_value(explicit).unwrap();
        assert_eq!(supplied.activity_emoji, Some(None));
        assert_eq!(supplied.mood_emoji, Some(Some("😌".into())));
        assert_eq!(
            supplied.dominant_emoji_kind,
            Some(Some(SummaryEmojiKind::Mood))
        );
    }
}
