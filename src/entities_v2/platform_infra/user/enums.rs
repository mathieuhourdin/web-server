use crate::entities_v2::error::{ErrorType, PpdcError};
use diesel::deserialize::{self, FromSql};
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::{SmallInt, Text};
use diesel::{AsExpression, FromSqlRow};
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};
use chrono::Weekday;

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsExpression, FromSqlRow)]
#[diesel(sql_type = diesel::sql_types::Text)]
pub enum JournalTheme {
    Classic,
    White,
    Flowers,
    Dark,
}

impl JournalTheme {
    pub fn to_code(self) -> &'static str {
        match self {
            JournalTheme::Classic => "classic",
            JournalTheme::White => "white",
            JournalTheme::Flowers => "flowers",
            JournalTheme::Dark => "dark",
        }
    }

    pub fn from_code(code: &str) -> Result<Self, PpdcError> {
        match code {
            "classic" | "Classic" => Ok(JournalTheme::Classic),
            "default" | "Default" => Ok(JournalTheme::Classic),
            "white" | "White" => Ok(JournalTheme::White),
            "flowers" | "Flowers" => Ok(JournalTheme::Flowers),
            "dark" | "Dark" => Ok(JournalTheme::Dark),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Invalid journal_theme: {}", code),
            )),
        }
    }

    pub fn to_api_value(self) -> &'static str {
        match self {
            JournalTheme::Classic => "classic",
            JournalTheme::White => "white",
            JournalTheme::Flowers => "flowers",
            JournalTheme::Dark => "dark",
        }
    }
}

impl Serialize for JournalTheme {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_api_value())
    }
}

impl<'de> Deserialize<'de> for JournalTheme {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        JournalTheme::from_code(&value).map_err(|_| de::Error::custom("unknown journal_theme"))
    }
}

impl ToSql<Text, Pg> for JournalTheme {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        <str as ToSql<Text, Pg>>::to_sql(self.to_code(), out)
    }
}

impl FromSql<Text, Pg> for JournalTheme {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
        JournalTheme::from_code(value.as_str())
            .map_err(|_| "invalid journal_theme value in database".into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsExpression, FromSqlRow)]
#[diesel(sql_type = diesel::sql_types::SmallInt)]
pub enum WeekAnalysisWeekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl WeekAnalysisWeekday {
    pub fn to_db_value(self) -> i16 {
        match self {
            WeekAnalysisWeekday::Monday => 1,
            WeekAnalysisWeekday::Tuesday => 2,
            WeekAnalysisWeekday::Wednesday => 3,
            WeekAnalysisWeekday::Thursday => 4,
            WeekAnalysisWeekday::Friday => 5,
            WeekAnalysisWeekday::Saturday => 6,
            WeekAnalysisWeekday::Sunday => 7,
        }
    }

    pub fn from_db_value(value: i16) -> Result<Self, PpdcError> {
        match value {
            1 => Ok(WeekAnalysisWeekday::Monday),
            2 => Ok(WeekAnalysisWeekday::Tuesday),
            3 => Ok(WeekAnalysisWeekday::Wednesday),
            4 => Ok(WeekAnalysisWeekday::Thursday),
            5 => Ok(WeekAnalysisWeekday::Friday),
            6 => Ok(WeekAnalysisWeekday::Saturday),
            7 => Ok(WeekAnalysisWeekday::Sunday),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Invalid week_analysis_weekday: {}", value),
            )),
        }
    }

    pub fn from_code(code: &str) -> Result<Self, PpdcError> {
        match code {
            "1" => Ok(WeekAnalysisWeekday::Monday),
            "2" => Ok(WeekAnalysisWeekday::Tuesday),
            "3" => Ok(WeekAnalysisWeekday::Wednesday),
            "4" => Ok(WeekAnalysisWeekday::Thursday),
            "5" => Ok(WeekAnalysisWeekday::Friday),
            "6" => Ok(WeekAnalysisWeekday::Saturday),
            "7" => Ok(WeekAnalysisWeekday::Sunday),
            "MONDAY" | "Monday" | "monday" => Ok(WeekAnalysisWeekday::Monday),
            "TUESDAY" | "Tuesday" | "tuesday" => Ok(WeekAnalysisWeekday::Tuesday),
            "WEDNESDAY" | "Wednesday" | "wednesday" => Ok(WeekAnalysisWeekday::Wednesday),
            "THURSDAY" | "Thursday" | "thursday" => Ok(WeekAnalysisWeekday::Thursday),
            "FRIDAY" | "Friday" | "friday" => Ok(WeekAnalysisWeekday::Friday),
            "SATURDAY" | "Saturday" | "saturday" => Ok(WeekAnalysisWeekday::Saturday),
            "SUNDAY" | "Sunday" | "sunday" => Ok(WeekAnalysisWeekday::Sunday),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Invalid week_analysis_weekday: {}", code),
            )),
        }
    }

    pub fn to_api_value(self) -> &'static str {
        match self {
            WeekAnalysisWeekday::Monday => "monday",
            WeekAnalysisWeekday::Tuesday => "tuesday",
            WeekAnalysisWeekday::Wednesday => "wednesday",
            WeekAnalysisWeekday::Thursday => "thursday",
            WeekAnalysisWeekday::Friday => "friday",
            WeekAnalysisWeekday::Saturday => "saturday",
            WeekAnalysisWeekday::Sunday => "sunday",
        }
    }

    pub fn to_chrono_weekday(self) -> Weekday {
        match self {
            WeekAnalysisWeekday::Monday => Weekday::Mon,
            WeekAnalysisWeekday::Tuesday => Weekday::Tue,
            WeekAnalysisWeekday::Wednesday => Weekday::Wed,
            WeekAnalysisWeekday::Thursday => Weekday::Thu,
            WeekAnalysisWeekday::Friday => Weekday::Fri,
            WeekAnalysisWeekday::Saturday => Weekday::Sat,
            WeekAnalysisWeekday::Sunday => Weekday::Sun,
        }
    }
}

impl Serialize for WeekAnalysisWeekday {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_api_value())
    }
}

impl<'de> Deserialize<'de> for WeekAnalysisWeekday {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        WeekAnalysisWeekday::from_code(&value)
            .map_err(|_| de::Error::custom("unknown week_analysis_weekday"))
    }
}

impl ToSql<SmallInt, Pg> for WeekAnalysisWeekday {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = self.to_db_value();
        <i16 as ToSql<SmallInt, Pg>>::to_sql(&value, &mut out.reborrow())
    }
}

impl FromSql<SmallInt, Pg> for WeekAnalysisWeekday {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        let value = <i16 as FromSql<SmallInt, Pg>>::from_sql(bytes)?;
        WeekAnalysisWeekday::from_db_value(value)
            .map_err(|_| "invalid week_analysis_weekday value in database".into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsExpression, FromSqlRow)]
#[diesel(sql_type = diesel::sql_types::Text)]
pub enum HomeFocusView {
    Projects,
    Follows,
    Drafts,
}

impl HomeFocusView {
    pub fn to_code(self) -> &'static str {
        match self {
            HomeFocusView::Projects => "projects",
            HomeFocusView::Follows => "follows",
            HomeFocusView::Drafts => "drafts",
        }
    }

    pub fn from_code(code: &str) -> Result<Self, PpdcError> {
        match code {
            "projects" | "PROJECTS" | "Projects" => Ok(HomeFocusView::Projects),
            "follows" | "FOLLOWS" | "Follows" => Ok(HomeFocusView::Follows),
            "drafts" | "DRAFTS" | "Drafts" => Ok(HomeFocusView::Drafts),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Invalid home_focus_view: {}", code),
            )),
        }
    }

    pub fn to_api_value(self) -> &'static str {
        self.to_code()
    }
}

impl Serialize for HomeFocusView {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_api_value())
    }
}

impl<'de> Deserialize<'de> for HomeFocusView {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        HomeFocusView::from_code(&value).map_err(|_| de::Error::custom("unknown home_focus_view"))
    }
}

impl ToSql<Text, Pg> for HomeFocusView {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        <str as ToSql<Text, Pg>>::to_sql(self.to_code(), out)
    }
}

impl FromSql<Text, Pg> for HomeFocusView {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
        HomeFocusView::from_code(value.as_str())
            .map_err(|_| "invalid home_focus_view value in database".into())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    Member,
    Admin,
    Mentor,
}

impl UserRole {
    pub fn to_db(self) -> &'static str {
        match self {
            UserRole::Member => "MEMBER",
            UserRole::Admin => "ADMIN",
            UserRole::Mentor => "MENTOR",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, PpdcError> {
        match value {
            "MEMBER" | "member" => Ok(UserRole::Member),
            "ADMIN" | "admin" => Ok(UserRole::Admin),
            "MENTOR" | "mentor" => Ok(UserRole::Mentor),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Invalid user role: {}", value),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsExpression, FromSqlRow)]
#[diesel(sql_type = diesel::sql_types::Text)]
pub enum UserPrincipalType {
    Human,
    Service,
}

impl UserPrincipalType {
    pub fn to_db(self) -> &'static str {
        match self {
            UserPrincipalType::Human => "HUMAN",
            UserPrincipalType::Service => "SERVICE",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, PpdcError> {
        match value {
            "HUMAN" | "human" => Ok(UserPrincipalType::Human),
            "SERVICE" | "service" => Ok(UserPrincipalType::Service),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Invalid user principal_type: {}", value),
            )),
        }
    }

    pub fn to_api_value(self) -> &'static str {
        match self {
            UserPrincipalType::Human => "human",
            UserPrincipalType::Service => "service",
        }
    }
}

impl Serialize for UserPrincipalType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_api_value())
    }
}

impl<'de> Deserialize<'de> for UserPrincipalType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        UserPrincipalType::from_db(&value).map_err(|_| de::Error::custom("unknown principal_type"))
    }
}

impl ToSql<Text, Pg> for UserPrincipalType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        <str as ToSql<Text, Pg>>::to_sql(self.to_db(), out)
    }
}

impl FromSql<Text, Pg> for UserPrincipalType {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
        UserPrincipalType::from_db(value.as_str())
            .map_err(|_| "invalid principal_type value in database".into())
    }
}
