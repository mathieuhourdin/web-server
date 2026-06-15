use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct UrlPreviewRequest {
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct UrlPreviewResponse {
    pub requested_url: String,
    pub canonical_url: String,
    pub source_kind: String,
    pub provider_name: String,
    pub content_type: String,
    pub title: String,
    pub description: String,
    pub image_url: Option<String>,
    pub site_name: Option<String>,
}
