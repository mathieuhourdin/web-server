use crate::entities::error::{ErrorType, PpdcError};
use diesel::backend::Backend;
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
pub enum ResourceType {
    Book,
    ReadingNote,
    ResourceList,
    Problem,
    ResearchArticle,
    NewsArticle,
    OpinionArticle,
    Movie,
    Video,
    Podcast,
    Song,
    Course,
    Idea,
    Journal,
    JournalItem,
    Trace,
    Mission,
    Element,
    Task,
    Question,
    Deliverable,
    Process,
    Resource,
    Analysis,
}

impl ResourceType {
    pub fn from_code(code: &str) -> Result<ResourceType, PpdcError> {
        dbg!(code);
        match code {
            "book" => Ok(ResourceType::Book),
            "rdnt" => Ok(ResourceType::ReadingNote),
            "list" => Ok(ResourceType::ResourceList),
            "pblm" => Ok(ResourceType::Problem),
            "ratc" => Ok(ResourceType::ResearchArticle),
            "natc" => Ok(ResourceType::NewsArticle),
            "oatc" => Ok(ResourceType::OpinionArticle),
            "movi" => Ok(ResourceType::Movie),
            "vide" => Ok(ResourceType::Video),
            "pcst" => Ok(ResourceType::Podcast),
            "song" => Ok(ResourceType::Song),
            "curs" => Ok(ResourceType::Course),
            "idea" => Ok(ResourceType::Idea),
            "jrnl" => Ok(ResourceType::Journal),
            "jrit" => Ok(ResourceType::JournalItem),
            "trce" => Ok(ResourceType::Trace),
            "atcl" => Ok(ResourceType::OpinionArticle),
            "miss" => Ok(ResourceType::Mission),
            "elmt" => Ok(ResourceType::Element),
            "task" => Ok(ResourceType::Task),
            "qest" => Ok(ResourceType::Question),
            "dlvr" => Ok(ResourceType::Deliverable),
            "proc" => Ok(ResourceType::Process),
            "anly" => Ok(ResourceType::Analysis),
            "rsrc" => Ok(ResourceType::Resource),
            &_ => {
                return Err(PpdcError::new(
                    404,
                    ErrorType::ApiError,
                    "resource_type not found".to_string(),
                ))
            }
        }
    }
    pub fn to_code(&self) -> &str {
        match self {
            ResourceType::Book => "book",
            ResourceType::ReadingNote => "rdnt",
            ResourceType::ResourceList => "list",
            ResourceType::Problem => "pblm",
            ResourceType::ResearchArticle => "ratc",
            ResourceType::NewsArticle => "natc",
            ResourceType::OpinionArticle => "oatc",
            ResourceType::Movie => "movi",
            ResourceType::Video => "vide",
            ResourceType::Podcast => "pcst",
            ResourceType::Song => "song",
            ResourceType::Course => "curs",
            ResourceType::Idea => "idea",
            ResourceType::Journal => "jrnl",
            ResourceType::JournalItem => "jrit",
            ResourceType::Trace => "trce",
            ResourceType::Mission => "miss",
            ResourceType::Element => "elmt",
            ResourceType::Task => "task",
            ResourceType::Question => "qest",
            ResourceType::Deliverable => "dlvr",
            ResourceType::Process => "proc",
            ResourceType::Analysis => "anly",
            ResourceType::Resource => "rsrc",
        }
    }
    pub fn to_full_text(&self) -> &str {
        match self {
            ResourceType::Book => "Book",
            ResourceType::ReadingNote => "Reading Note",
            ResourceType::ResourceList => "List",
            ResourceType::Problem => "Problem",
            ResourceType::ResearchArticle => "Research Article",
            ResourceType::NewsArticle => "News Article",
            ResourceType::OpinionArticle => "Opinion Article",
            ResourceType::Movie => "Movie",
            ResourceType::Video => "Video",
            ResourceType::Podcast => "Podcast",
            ResourceType::Song => "Song",
            ResourceType::Course => "Course",
            ResourceType::Idea => "Idea",
            ResourceType::Journal => "Journal",
            ResourceType::JournalItem => "Journal Item",
            ResourceType::Trace => "Trace",
            ResourceType::Mission => "Mission",
            ResourceType::Element => "Element",
            ResourceType::Task => "Task",
            ResourceType::Question => "Question",
            ResourceType::Deliverable => "Deliverable",
            ResourceType::Process => "Process",
            ResourceType::Analysis => "Analysis",
            ResourceType::Resource => "Resource",
        }
    }
    pub fn from_full_text(full_text: &str) -> Option<ResourceType> {
        match full_text {
            "Book" => Some(ResourceType::Book),
            "Reading Note" => Some(ResourceType::ReadingNote),
            "List" => Some(ResourceType::ResourceList),
            "Problem" => Some(ResourceType::Problem),
            "Research Article" => Some(ResourceType::ResearchArticle),
            "News Article" => Some(ResourceType::NewsArticle),
            "Opinion Article" => Some(ResourceType::OpinionArticle),
            "Movie" => Some(ResourceType::Movie),
            "Video" => Some(ResourceType::Video),
            "Podcast" => Some(ResourceType::Podcast),
            "Song" => Some(ResourceType::Song),
            "Course" => Some(ResourceType::Course),
            "Idea" => Some(ResourceType::Idea),
            "Journal" => Some(ResourceType::Journal),
            "Journal Item" => Some(ResourceType::JournalItem),
            "Trace" => Some(ResourceType::Trace),
            "Mission" => Some(ResourceType::Mission),
            "Element" => Some(ResourceType::Element),
            "Task" => Some(ResourceType::Task),
            "Question" => Some(ResourceType::Question),
            "Deliverable" => Some(ResourceType::Deliverable),
            "Process" => Some(ResourceType::Process),
            "Analysis" => Some(ResourceType::Analysis),
            "Resource" => Some(ResourceType::Resource),
            &_ => None,
        }
    }
    pub fn from_opengraph_code(og_code: &str) -> Option<ResourceType> {
        match og_code {
            "book" => Some(ResourceType::Book),
            "article" => Some(ResourceType::NewsArticle),
            "music.song" => Some(ResourceType::Podcast),
            &_ => None,
        }
    }
}

impl Serialize for ResourceType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_code())
    }
}

impl<'de> Deserialize<'de> for ResourceType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ResourceType::from_code(&s).map_err(|_err| de::Error::custom("unknown resource_type"))
    }
}

impl ToSql<Text, Pg> for ResourceType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        <str as ToSql<Text, Pg>>::to_sql(self.to_code(), out)
    }
}

impl<DB> FromSql<Text, DB> for ResourceType
where
    DB: for<'b> Backend<RawValue<'b> = PgValue<'b>>,
    String: ToSql<Text, diesel::pg::Pg>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> deserialize::Result<Self> {
        Ok(ResourceType::from_code(String::from_sql(bytes)?.as_str()).unwrap())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_from_code() {
        let from_code_result = ResourceType::from_code("book");
        assert_eq!(from_code_result, Ok(ResourceType::Book));
    }

    #[test]
    fn test_unknown_code() {
        let from_code_result = ResourceType::from_code("i_dont_exist");
        let error = from_code_result.expect_err("Code should not be found");
        assert_eq!(
            error,
            PpdcError::new(
                404,
                ErrorType::ApiError,
                "resource_type not found".to_string()
            )
        )
    }

    #[test]
    fn test_to_code() {
        let resource_type = ResourceType::Book;
        let to_code_result = resource_type.to_code();
        assert_eq!(to_code_result, "book");
    }

    #[test]
    fn serialize_resource_type() {
        let resource_type = ResourceType::Book;
        let serialized = serde_json::to_string(&resource_type).unwrap();
        assert_eq!(serialized, "\"book\"");
    }

    #[test]
    fn deserialize_resource_type() {
        let serialized = "\"book\"";
        println!("Serialized : {}", serialized);
        let deserialized = serde_json::from_str::<ResourceType>(serialized);
        let de_content = deserialized.expect("Deserialization shouldn't fail");
        assert_eq!(de_content, ResourceType::Book);
    }
}
