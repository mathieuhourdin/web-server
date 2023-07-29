use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Article {
    pub uuid: Option<String>,
    pub title: String,
    pub description: String,
    pub content: String,
    pub author_id: Option<String>,
    pub progress: u32,
    pub gdoc_url: String,
    pub image_url: String,
    pub url_slug: String,
}
