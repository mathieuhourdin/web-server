use crate::entities::error::{ErrorType, PpdcError};
use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::pg::PgValue;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Text;
use diesel::AsExpression;
use diesel::FromSqlRow;
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};

#[derive(Clone, Debug, Copy, AsExpression, PartialEq, FromSqlRow)]
#[diesel(sql_type = diesel::sql_types::Text)]
pub enum EntityType {
    Journal,
    Trace,
    TraceMirror,
    TraceMirrorJournal,
    TraceMirrorBio,
    TraceMirrorHighLevelProjectsDefinition,
    Element,
    Landmark,
    LandscapeAnalysis,
    Lens,
    PublicPost,
}

impl EntityType {
    pub fn from_code(code: &str) -> Result<EntityType, PpdcError> {
        match code {
            "jrnl" => Ok(EntityType::Journal),
            "trce" => Ok(EntityType::Trace),
            "trcm" => Ok(EntityType::TraceMirror),
            "trmj" => Ok(EntityType::TraceMirrorJournal),
            "trmb" => Ok(EntityType::TraceMirrorBio),
            "trmh" => Ok(EntityType::TraceMirrorHighLevelProjectsDefinition),
            "elmt" => Ok(EntityType::Element),
            "lndm" => Ok(EntityType::Landmark),
            "lnds" => Ok(EntityType::LandscapeAnalysis),
            "lens" => Ok(EntityType::Lens),
            "ppst" => Ok(EntityType::PublicPost),
            &_ => {
                return Err(PpdcError::new(
                    404,
                    ErrorType::ApiError,
                    "entity_type not found".to_string(),
                ))
            }
        }
    }

    pub fn to_code(&self) -> &str {
        match self {
            EntityType::Journal => "jrnl",
            EntityType::Trace => "trce",
            EntityType::TraceMirror => "trcm",
            EntityType::TraceMirrorJournal => "trmj",
            EntityType::TraceMirrorBio => "trmb",
            EntityType::TraceMirrorHighLevelProjectsDefinition => "trmh",
            EntityType::Element => "elmt",
            EntityType::Landmark => "lndm",
            EntityType::LandscapeAnalysis => "lnds",
            EntityType::Lens => "lens",
            EntityType::PublicPost => "ppst",
        }
    }

    pub fn to_full_text(&self) -> &str {
        match self {
            EntityType::Journal => "Journal",
            EntityType::Trace => "Trace",
            EntityType::TraceMirror => "Trace Mirror Note",
            EntityType::TraceMirrorJournal => "Trace Mirror Journal",
            EntityType::TraceMirrorBio => "Trace Mirror Bio",
            EntityType::TraceMirrorHighLevelProjectsDefinition => {
                "Trace Mirror High Level Projects"
            }
            EntityType::Element => "Element",
            EntityType::Landmark => "Landmark",
            EntityType::LandscapeAnalysis => "Landscape Analysis",
            EntityType::Lens => "Lens",
            EntityType::PublicPost => "Public Post",
        }
    }

    pub fn from_full_text(full_text: &str) -> Result<EntityType, PpdcError> {
        match full_text {
            "Journal" => Ok(EntityType::Journal),
            "Trace" => Ok(EntityType::Trace),
            "Trace Mirror" => Ok(EntityType::TraceMirror),
            "Trace Mirror Note" => Ok(EntityType::TraceMirror),
            "Trace Mirror Journal" => Ok(EntityType::TraceMirrorJournal),
            "Trace Mirror Bio" => Ok(EntityType::TraceMirrorBio),
            "Trace Mirror High Level Projects Definition" => {
                Ok(EntityType::TraceMirrorHighLevelProjectsDefinition)
            }
            "Trace Mirror High Level Projects" => {
                Ok(EntityType::TraceMirrorHighLevelProjectsDefinition)
            }
            "Element" => Ok(EntityType::Element),
            "Landmark" => Ok(EntityType::Landmark),
            "Landscape Analysis" => Ok(EntityType::LandscapeAnalysis),
            "Lens" => Ok(EntityType::Lens),
            "Public Post" => Ok(EntityType::PublicPost),
            &_ => Err(PpdcError::new(
                404,
                ErrorType::ApiError,
                "entity_type not found".to_string(),
            )),
        }
    }
}

impl Serialize for EntityType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_code())
    }
}

impl<'de> Deserialize<'de> for EntityType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        EntityType::from_code(&s).map_err(|_err| de::Error::custom("unknown entity_type"))
    }
}

impl ToSql<Text, Pg> for EntityType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        <str as ToSql<Text, Pg>>::to_sql(self.to_code(), out)
    }
}

impl FromSql<Text, Pg> for EntityType {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        let s = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
        Ok(EntityType::from_code(s.as_str()).unwrap())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_from_code() {
        let from_code_result = EntityType::from_code("jrnl");
        assert_eq!(from_code_result, Ok(EntityType::Journal));
    }

    #[test]
    fn test_unknown_code() {
        let from_code_result = EntityType::from_code("i_dont_exist");
        let error = from_code_result.expect_err("Code should not be found");
        assert_eq!(
            error,
            PpdcError::new(
                404,
                ErrorType::ApiError,
                "entity_type not found".to_string()
            )
        )
    }

    #[test]
    fn test_to_code() {
        let entity_type = EntityType::Journal;
        let to_code_result = entity_type.to_code();
        assert_eq!(to_code_result, "jrnl");
    }

    #[test]
    fn serialize_entity_type() {
        let entity_type = EntityType::Journal;
        let serialized = serde_json::to_string(&entity_type).unwrap();
        assert_eq!(serialized, "\"jrnl\"");
    }

    #[test]
    fn deserialize_entity_type() {
        let serialized = "\"jrnl\"";
        let deserialized = serde_json::from_str::<EntityType>(serialized);
        let de_content = deserialized.expect("Deserialization shouldn't fail");
        assert_eq!(de_content, EntityType::Journal);
    }
}
