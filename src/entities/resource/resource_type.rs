use crate::entities::error::{ErrorType, PpdcError};
use crate::entities::resource::entity_type::EntityType;
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
    Book,               // public post
    ReadingNote,        // public post
    ResourceList,       // public post
    Problem,            // public post
    ResearchArticle,    //public post
    NewsArticle,        // public post
    OpinionArticle,     // public post
    Movie,              // public post
    Video,              // public post
    Podcast,            // public post
    Song,               // public post
    Course,             // public post
    Idea,               // public post
    Journal,            // Journal
    WorkLogJournal,     // Journal
    ReadingNoteJournal, // Journal
    JournalItem,        // public post
    Trace,              // Legacy Trace
    UserTrace,          // Trace
    BioTrace,           // Trace
    WorkspaceTrace,     // Trace
    Mission,            // Landmark
    Element,            // Element
    Task,               // Landmark
    Question,           // Landmark
    Deliverable,        // Landmark
    Process,            // Landmark
    Resource,           // Landmark
    Analysis,           // LandscapeAnalysis
    Event,              // Element
    GeneralComment,     // Element
    Feeling,            // Element
    Theme,              // Landmark
    Lens,               // Lens
    TraceMirror,        // TraceMirror
    Author,             // Landmark
    MetaJournal,        // Journal
}

impl ResourceType {
    pub fn from_code(code: &str) -> Result<ResourceType, PpdcError> {
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
            "wjnl" => Ok(ResourceType::WorkLogJournal),
            "rjnl" => Ok(ResourceType::ReadingNoteJournal),
            "jrit" => Ok(ResourceType::JournalItem),
            "trce" => Ok(ResourceType::UserTrace),
            "utrc" => Ok(ResourceType::UserTrace),
            "btrc" => Ok(ResourceType::BioTrace),
            "wtrc" => Ok(ResourceType::WorkspaceTrace),
            "atcl" => Ok(ResourceType::OpinionArticle),
            "miss" => Ok(ResourceType::Mission),
            "elmt" => Ok(ResourceType::Element),
            "task" => Ok(ResourceType::Task),
            "qest" => Ok(ResourceType::Question),
            "dlvr" => Ok(ResourceType::Deliverable),
            "proc" => Ok(ResourceType::Process),
            "anly" => Ok(ResourceType::Analysis),
            "rsrc" => Ok(ResourceType::Resource),
            "evnt" => Ok(ResourceType::Event),
            "cmnt" => Ok(ResourceType::GeneralComment),
            "feln" => Ok(ResourceType::Feeling),
            "them" => Ok(ResourceType::Theme),
            "lens" => Ok(ResourceType::Lens),
            "trcm" => Ok(ResourceType::TraceMirror),
            "autr" => Ok(ResourceType::Author),
            "meta" => Ok(ResourceType::MetaJournal),
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
            ResourceType::WorkLogJournal => "wjnl",
            ResourceType::ReadingNoteJournal => "rjnl",
            ResourceType::JournalItem => "jrit",
            ResourceType::Trace => "trce",
            ResourceType::UserTrace => "trce",
            ResourceType::BioTrace => "btrc",
            ResourceType::WorkspaceTrace => "wtrc",
            ResourceType::Mission => "miss",
            ResourceType::Element => "elmt",
            ResourceType::Task => "task",
            ResourceType::Question => "qest",
            ResourceType::Deliverable => "dlvr",
            ResourceType::Process => "proc",
            ResourceType::Analysis => "anly",
            ResourceType::Resource => "rsrc",
            ResourceType::Event => "evnt",
            ResourceType::GeneralComment => "cmnt",
            ResourceType::Feeling => "feln",
            ResourceType::Theme => "them",
            ResourceType::Lens => "lens",
            ResourceType::TraceMirror => "trcm",
            ResourceType::Author => "autr",
            ResourceType::MetaJournal => "meta",
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
            ResourceType::WorkLogJournal => "Work Log Journal",
            ResourceType::ReadingNoteJournal => "Reading Note Journal",
            ResourceType::JournalItem => "Journal Item",
            ResourceType::Trace => "Trace",
            ResourceType::UserTrace => "User Trace",
            ResourceType::BioTrace => "Bio Trace",
            ResourceType::WorkspaceTrace => "Workspace Trace",
            ResourceType::Mission => "Mission",
            ResourceType::Element => "Element",
            ResourceType::Task => "Task",
            ResourceType::Question => "Question",
            ResourceType::Deliverable => "Deliverable",
            ResourceType::Process => "Process",
            ResourceType::Analysis => "Analysis",
            ResourceType::Resource => "Resource",
            ResourceType::Event => "Event",
            ResourceType::GeneralComment => "GeneralComment",
            ResourceType::Feeling => "Feeling",
            ResourceType::Theme => "Theme",
            ResourceType::Lens => "Lens",
            ResourceType::TraceMirror => "Trace Mirror",
            ResourceType::Author => "Author",
            ResourceType::MetaJournal => "Meta Journal",
        }
    }
    pub fn from_full_text(full_text: &str) -> Result<ResourceType, PpdcError> {
        match full_text {
            "Book" => Ok(ResourceType::Book),
            "Reading Note" => Ok(ResourceType::ReadingNote),
            "List" => Ok(ResourceType::ResourceList),
            "Problem" => Ok(ResourceType::Problem),
            "Research Article" => Ok(ResourceType::ResearchArticle),
            "News Article" => Ok(ResourceType::NewsArticle),
            "Opinion Article" => Ok(ResourceType::OpinionArticle),
            "Movie" => Ok(ResourceType::Movie),
            "Video" => Ok(ResourceType::Video),
            "Podcast" => Ok(ResourceType::Podcast),
            "Song" => Ok(ResourceType::Song),
            "Course" => Ok(ResourceType::Course),
            "Idea" => Ok(ResourceType::Idea),
            "Journal" => Ok(ResourceType::Journal),
            "Work Log Journal" => Ok(ResourceType::WorkLogJournal),
            "Reading Note Journal" => Ok(ResourceType::ReadingNoteJournal),
            "Journal Item" => Ok(ResourceType::JournalItem),
            "Trace" => Ok(ResourceType::UserTrace),
            "User Trace" => Ok(ResourceType::UserTrace),
            "Bio Trace" => Ok(ResourceType::BioTrace),
            "Workspace Trace" => Ok(ResourceType::WorkspaceTrace),
            "Mission" => Ok(ResourceType::Mission),
            "Element" => Ok(ResourceType::Element),
            "Task" => Ok(ResourceType::Task),
            "Question" => Ok(ResourceType::Question),
            "Deliverable" => Ok(ResourceType::Deliverable),
            "Process" => Ok(ResourceType::Process),
            "Analysis" => Ok(ResourceType::Analysis),
            "Resource" => Ok(ResourceType::Resource),
            "Event" => Ok(ResourceType::Event),
            "GeneralComment" => Ok(ResourceType::GeneralComment),
            "Feeling" => Ok(ResourceType::Feeling),
            "Theme" => Ok(ResourceType::Theme),
            "Lens" => Ok(ResourceType::Lens),
            "Trace Mirror" => Ok(ResourceType::TraceMirror),
            "Author" => Ok(ResourceType::Author),
            "Meta Journal" => Ok(ResourceType::MetaJournal),
            &_ => Err(PpdcError::new(
                404,
                ErrorType::ApiError,
                "resource_type not found".to_string(),
            )),
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

    pub fn to_entity_type(&self) -> EntityType {
        match self {
            ResourceType::Book
            | ResourceType::ReadingNote
            | ResourceType::ResourceList
            | ResourceType::Problem
            | ResourceType::ResearchArticle
            | ResourceType::NewsArticle
            | ResourceType::OpinionArticle
            | ResourceType::Movie
            | ResourceType::Video
            | ResourceType::Podcast
            | ResourceType::Song
            | ResourceType::Course
            | ResourceType::Idea
            | ResourceType::JournalItem => EntityType::PublicPost,
            ResourceType::Journal
            | ResourceType::WorkLogJournal
            | ResourceType::ReadingNoteJournal => EntityType::Journal,
            ResourceType::Trace
            | ResourceType::UserTrace
            | ResourceType::BioTrace
            | ResourceType::WorkspaceTrace => EntityType::Trace,
            ResourceType::TraceMirror => EntityType::TraceMirror,
            ResourceType::Element
            | ResourceType::Event
            | ResourceType::GeneralComment
            | ResourceType::Feeling => EntityType::Element,
            ResourceType::Mission
            | ResourceType::Task
            | ResourceType::Question
            | ResourceType::Deliverable
            | ResourceType::Process
            | ResourceType::Resource
            | ResourceType::Theme
            | ResourceType::Author => EntityType::Landmark,
            ResourceType::Analysis => EntityType::LandscapeAnalysis,
            ResourceType::Lens => EntityType::Lens,
            ResourceType::MetaJournal => EntityType::Journal,
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

impl FromSql<Text, Pg> for ResourceType {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        let s = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
        Ok(ResourceType::from_code(s.as_str()).unwrap())
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
