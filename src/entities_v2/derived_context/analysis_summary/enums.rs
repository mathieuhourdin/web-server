use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SummaryEmojiKind {
    Activity,
    Mood,
}

impl SummaryEmojiKind {
    pub fn to_db(self) -> &'static str {
        match self {
            Self::Activity => "ACTIVITY",
            Self::Mood => "MOOD",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "ACTIVITY" => Some(Self::Activity),
            "MOOD" => Some(Self::Mood),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisSummaryType {
    PeriodRecap,
}

impl AnalysisSummaryType {
    pub fn to_db(self) -> &'static str {
        match self {
            AnalysisSummaryType::PeriodRecap => "PERIOD_RECAP",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "PERIOD_RECAP" | "period_recap" => AnalysisSummaryType::PeriodRecap,
            _ => AnalysisSummaryType::PeriodRecap,
        }
    }
}
