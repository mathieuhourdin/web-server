use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct Article {
    pub uuid: Option<String>,
    pub title: String,
    pub content: String,
}
