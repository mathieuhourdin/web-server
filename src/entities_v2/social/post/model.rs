use chrono::NaiveDateTime;
use serde::de::Deserializer;
use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;

use crate::entities_v2::shared::MaturingState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostType {
    Output,
    Input,
    Problem,
    Wish,
}

impl PostType {
    pub fn to_code(self) -> &'static str {
        match self {
            PostType::Output => "outp",
            PostType::Input => "inpt",
            PostType::Problem => "pblm",
            PostType::Wish => "wish",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "inpt" | "Input" | "input" => PostType::Input,
            "pblm" | "Problem" | "problem" => PostType::Problem,
            "wish" | "Wish" => PostType::Wish,
            _ => PostType::Output,
        }
    }

    pub fn to_interaction_type(self) -> &'static str {
        self.to_code()
    }

    pub fn from_interaction_type(interaction_type: &str) -> Self {
        Self::from_code(interaction_type)
    }

    pub fn to_db(self) -> &'static str {
        match self {
            PostType::Output => "OUTPUT",
            PostType::Input => "INPUT",
            PostType::Problem => "PROBLEM",
            PostType::Wish => "WISH",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "INPUT" | "input" => PostType::Input,
            "PROBLEM" | "problem" => PostType::Problem,
            "WISH" | "wish" => PostType::Wish,
            _ => PostType::Output,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Post {
    pub id: Uuid,
    #[serde(skip_serializing, skip_deserializing)]
    pub resource_id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub image_url: Option<String>,
    pub post_type: PostType,
    pub user_id: Uuid,
    pub publishing_date: Option<NaiveDateTime>,
    pub publishing_state: String,
    pub maturing_state: MaturingState,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewPostDto {
    pub title: String,
    pub subtitle: Option<String>,
    pub content: String,
    pub image_url: Option<String>,
    pub post_type: Option<PostType>,
    pub publishing_date: Option<NaiveDateTime>,
    pub publishing_state: Option<String>,
    pub maturing_state: Option<MaturingState>,
}

#[derive(Debug, Clone)]
pub struct NewPost {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub image_url: Option<String>,
    pub post_type: PostType,
    pub user_id: Uuid,
    pub publishing_date: Option<NaiveDateTime>,
    pub publishing_state: String,
    pub maturing_state: MaturingState,
}

impl NewPost {
    pub fn new(payload: NewPostDto, user_id: Uuid) -> Self {
        Self {
            title: payload.title,
            subtitle: payload.subtitle.unwrap_or_default(),
            content: payload.content,
            image_url: payload.image_url,
            post_type: payload.post_type.unwrap_or(PostType::Output),
            user_id,
            publishing_date: payload.publishing_date,
            publishing_state: payload
                .publishing_state
                .unwrap_or_else(|| "pbsh".to_string()),
            maturing_state: payload.maturing_state.unwrap_or(MaturingState::Draft),
        }
    }
}
