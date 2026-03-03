use crate::entities_v2::error::{ErrorType, PpdcError};
use diesel::deserialize::{self, FromSql};
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Text;
use diesel::{AsExpression, FromSqlRow};
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};

#[derive(Clone, Debug, Copy, AsExpression, PartialEq, Eq, FromSqlRow)]
#[diesel(sql_type = diesel::sql_types::Text)]
pub enum MaturingState {
    Draft,
    Review,
    Finished,
    Trashed,
    Replay,
}

impl MaturingState {
    pub fn from_code(code: &str) -> Result<MaturingState, PpdcError> {
        match code {
            "drft" => Ok(MaturingState::Draft),
            "rvew" => Ok(MaturingState::Review),
            "fnsh" => Ok(MaturingState::Finished),
            "trsh" => Ok(MaturingState::Trashed),
            "rply" => Ok(MaturingState::Replay),
            _ => Err(PpdcError::new(
                404,
                ErrorType::ApiError,
                "maturing_state not found".to_string(),
            )),
        }
    }

    pub fn to_code(&self) -> &str {
        match self {
            MaturingState::Draft => "drft",
            MaturingState::Review => "rvew",
            MaturingState::Finished => "fnsh",
            MaturingState::Trashed => "trsh",
            MaturingState::Replay => "rply",
        }
    }
}

impl Serialize for MaturingState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_code())
    }
}

impl<'de> Deserialize<'de> for MaturingState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        MaturingState::from_code(&s).map_err(|_| de::Error::custom("unknown maturing_state"))
    }
}

impl ToSql<Text, Pg> for MaturingState {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        <str as ToSql<Text, Pg>>::to_sql(self.to_code(), out)
    }
}

impl FromSql<Text, Pg> for MaturingState {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        let s = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
        Ok(MaturingState::from_code(s.as_str()).unwrap_or(MaturingState::Draft))
    }
}
