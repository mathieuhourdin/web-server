use uuid::Uuid;
use chrono::NaiveDateTime;

pub struct Element {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub extended_content: Option<String>,
    pub element_type: ElementType,
    pub user_id: Uuid,
    pub analysis_id: Uuid,
    pub trace_id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,

}

pub enum ElementType {
    Action,
    Event,
    Idea,
    Quote,
    Emotion,
    Fact,
}
