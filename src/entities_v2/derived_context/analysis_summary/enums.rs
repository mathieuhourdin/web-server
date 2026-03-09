use serde::{Deserialize, Serialize};

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
