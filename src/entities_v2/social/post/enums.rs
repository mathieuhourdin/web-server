use serde::de::Deserializer;
use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostStatus {
    Draft,
    Published,
    Archived,
}

impl PostStatus {
    pub fn to_code(self) -> &'static str {
        match self {
            PostStatus::Draft => "draft",
            PostStatus::Published => "published",
            PostStatus::Archived => "archived",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "published" | "PUBLISHED" | "Published" => PostStatus::Published,
            "archived" | "ARCHIVED" | "Archived" => PostStatus::Archived,
            _ => PostStatus::Draft,
        }
    }

    pub fn to_db(self) -> &'static str {
        match self {
            PostStatus::Draft => "DRAFT",
            PostStatus::Published => "PUBLISHED",
            PostStatus::Archived => "ARCHIVED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "PUBLISHED" | "published" => PostStatus::Published,
            "ARCHIVED" | "archived" => PostStatus::Archived,
            _ => PostStatus::Draft,
        }
    }
}

impl Serialize for PostStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_code())
    }
}

impl<'de> Deserialize<'de> for PostStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(PostStatus::from_code(&value))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostInteractionType {
    Output,
    Input,
    Problem,
    Wish,
}

impl PostInteractionType {
    pub fn to_code(self) -> &'static str {
        match self {
            PostInteractionType::Output => "outp",
            PostInteractionType::Input => "inpt",
            PostInteractionType::Problem => "pblm",
            PostInteractionType::Wish => "wish",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "inpt" | "Input" | "input" | "INPUT" => PostInteractionType::Input,
            "pblm" | "Problem" | "problem" | "PROBLEM" => PostInteractionType::Problem,
            "wish" | "Wish" | "WISH" => PostInteractionType::Wish,
            _ => PostInteractionType::Output,
        }
    }

    pub fn to_db(self) -> &'static str {
        match self {
            PostInteractionType::Output => "OUTPUT",
            PostInteractionType::Input => "INPUT",
            PostInteractionType::Problem => "PROBLEM",
            PostInteractionType::Wish => "WISH",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "INPUT" | "input" => PostInteractionType::Input,
            "PROBLEM" | "problem" => PostInteractionType::Problem,
            "WISH" | "wish" => PostInteractionType::Wish,
            _ => PostInteractionType::Output,
        }
    }
}

impl Serialize for PostInteractionType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_code())
    }
}

impl<'de> Deserialize<'de> for PostInteractionType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(PostInteractionType::from_code(&value))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostType {
    Idea,
    ResearchArticle,
    Book,
    Course,
    Question,
    OpinionArticle,
    Problem,
    Podcast,
    NewsArticle,
    ReadingNote,
    ResourceList,
}

impl PostType {
    pub fn to_code(self) -> &'static str {
        match self {
            PostType::Idea => "idea",
            PostType::ResearchArticle => "ratc",
            PostType::Book => "book",
            PostType::Course => "curs",
            PostType::Question => "quest",
            PostType::OpinionArticle => "oatc",
            PostType::Problem => "pblm",
            PostType::Podcast => "pcst",
            PostType::NewsArticle => "natc",
            PostType::ReadingNote => "rdnt",
            PostType::ResourceList => "list",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "ratc" | "RATC" => PostType::ResearchArticle,
            "book" | "BOOK" => PostType::Book,
            "curs" | "CURS" => PostType::Course,
            "quest" | "QUEST" | "qest" | "QEST" => PostType::Question,
            "oatc" | "OATC" => PostType::OpinionArticle,
            "pblm" | "PBLM" => PostType::Problem,
            "pcst" | "PCST" => PostType::Podcast,
            "natc" | "NATC" => PostType::NewsArticle,
            "rdnt" | "RDNT" => PostType::ReadingNote,
            "list" | "LIST" => PostType::ResourceList,
            _ => PostType::Idea,
        }
    }

    pub fn to_db(self) -> &'static str {
        match self {
            PostType::Idea => "IDEA",
            PostType::ResearchArticle => "RESEARCH_ARTICLE",
            PostType::Book => "BOOK",
            PostType::Course => "COURSE",
            PostType::Question => "QUESTION",
            PostType::OpinionArticle => "OPINION_ARTICLE",
            PostType::Problem => "PROBLEM",
            PostType::Podcast => "PODCAST",
            PostType::NewsArticle => "NEWS_ARTICLE",
            PostType::ReadingNote => "READING_NOTE",
            PostType::ResourceList => "RESOURCE_LIST",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "RESEARCH_ARTICLE" | "research_article" => PostType::ResearchArticle,
            "BOOK" | "book" => PostType::Book,
            "COURSE" | "course" => PostType::Course,
            "QUESTION" | "question" => PostType::Question,
            "OPINION_ARTICLE" | "opinion_article" => PostType::OpinionArticle,
            "PROBLEM" | "problem" => PostType::Problem,
            "PODCAST" | "podcast" => PostType::Podcast,
            "NEWS_ARTICLE" | "news_article" => PostType::NewsArticle,
            "READING_NOTE" | "reading_note" => PostType::ReadingNote,
            "RESOURCE_LIST" | "resource_list" => PostType::ResourceList,
            _ => PostType::Idea,
        }
    }
}

impl Serialize for PostType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_code())
    }
}

impl<'de> Deserialize<'de> for PostType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(PostType::from_code(&value))
    }
}
