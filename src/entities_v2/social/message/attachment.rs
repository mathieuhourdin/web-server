use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageAttachmentType {
    TarotReading,
}

impl MessageAttachmentType {
    pub fn to_db(self) -> &'static str {
        match self {
            MessageAttachmentType::TarotReading => "TAROT_READING",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "TAROT_READING" | "tarot_reading" => Some(MessageAttachmentType::TarotReading),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TarotSpreadType {
    ThreeCardTimeline,
    FourCardCross,
    NineCardTimelineGrid,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TarotCard {
    pub card_name: String,
    pub position: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TarotReadingAttachment {
    pub spread_type: TarotSpreadType,
    pub cards: Vec<TarotCard>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum MessageAttachment {
    TarotReading(TarotReadingAttachment),
}

impl MessageAttachment {
    pub fn attachment_type(&self) -> MessageAttachmentType {
        match self {
            MessageAttachment::TarotReading(_) => MessageAttachmentType::TarotReading,
        }
    }

    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        match self {
            MessageAttachment::TarotReading(payload) => serde_json::to_string(payload),
        }
    }

    pub fn from_json_string(attachment_type: MessageAttachmentType, value: &str) -> Option<Self> {
        match attachment_type {
            MessageAttachmentType::TarotReading => {
                serde_json::from_str::<TarotReadingAttachment>(value)
                    .ok()
                    .map(MessageAttachment::TarotReading)
            }
        }
    }
}
