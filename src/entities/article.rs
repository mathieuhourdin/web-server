use serde::{Serialize, Deserialize};
use std::time::{SystemTime};

#[derive(Serialize, Deserialize)]
pub struct Article {
    pub uuid: Option<String>,
    pub title: String,
    pub description: String,
    pub content: String,
    pub author_id: String,
    pub progress: u32,
    pub gdoc_link: String,
    pub image_url: String,
}
