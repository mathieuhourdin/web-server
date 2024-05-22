use serde::{Serialize, Serializer, Deserialize};
use serde::de::{self, Deserializer};
use crate::entities::error::{PpdcError, ErrorType};
use diesel::deserialize::{self, FromSql};
use diesel::serialize::{self, Output, ToSql};
use diesel::backend::Backend;
use diesel::pg::Pg;
use diesel::sql_types::Text;
use diesel::pg::PgValue;
use diesel::FromSqlRow;
use diesel::AsExpression;

#[derive(Clone, Debug, Copy, AsExpression, PartialEq, FromSqlRow)]
#[diesel(sql_type = diesel::sql_types::Text)]
pub enum MaturingState {
    Draft,
    Review,
    Finished
}

impl MaturingState {
    pub fn from_code(code: &str) -> Result<MaturingState, PpdcError> {
        dbg!(code);
        match code {
            "drft" => Ok(MaturingState::Draft),
            "rvew" => Ok(MaturingState::Review),
            "fnsh" => Ok(MaturingState::Finished),
            &_ => return Err(PpdcError::new(404, ErrorType::ApiError, "maturing_state not found".to_string()))
        }
    }
    pub fn to_code(&self) -> &str {
        match self {
            MaturingState::Draft => "drft",
            MaturingState::Review => "rvew",
            MaturingState::Finished => "fnsh",
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
        where D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        MaturingState::from_code(&s)
            .map_err(|_err| de::Error::custom("unknown maturing_state"))
    }
}

impl ToSql<Text, Pg> for MaturingState {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        <str as ToSql<Text, Pg>>::to_sql(self.to_code(), out)
    }
}

impl<DB> FromSql<Text, DB> for MaturingState 
where
    DB: for<'b> Backend<RawValue<'b> = PgValue<'b>>,
    String: ToSql<Text, diesel::pg::Pg>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> deserialize::Result<Self> 
    {
        Ok(MaturingState::from_code(String::from_sql(bytes)?.as_str()).unwrap())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_from_code() {
        let from_code_result = MaturingState::from_code("drft");
        assert_eq!(from_code_result, Ok(MaturingState::Draft));
    }

    #[test]
    fn test_unknown_code() {
        let from_code_result = MaturingState::from_code("i_dont_exist");
        let error = from_code_result.expect_err("Code should not be found");
        assert_eq!(error, PpdcError::new(404, ErrorType::ApiError, "maturing_state not found".to_string()))
    }

    #[test]
    fn test_to_code() {
        let resource_type = MaturingState::Draft;
        let to_code_result = resource_type.to_code();
        assert_eq!(to_code_result, "drft");
    }

    #[test]
    fn serialize_resource_type() {
        let resource_type = MaturingState::Draft;
        let serialized = serde_json::to_string(&resource_type).unwrap();
        assert_eq!(serialized, "\"drft\"");
    }

    #[test]
    fn deserialize_resource_type() {
        let serialized = "\"drft\"";
        println!("Serialized : {}", serialized);
        let deserialized = serde_json::from_str::<MaturingState>(serialized);
        let de_content = deserialized.expect("Deserialization shouldn't fail");
        assert_eq!(de_content, MaturingState::Draft);
    }
}
