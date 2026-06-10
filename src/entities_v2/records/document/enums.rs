use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentRole {
    Creation,
    Reference,
}

impl DocumentRole {
    pub fn to_db(self) -> &'static str {
        match self {
            DocumentRole::Creation => "CREATION",
            DocumentRole::Reference => "REFERENCE",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "REFERENCE" | "reference" => DocumentRole::Reference,
            _ => DocumentRole::Creation,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentContentSource {
    DbContent,
    InternalAsset,
    ExternalUrl,
    ReferenceOnly,
}

impl DocumentContentSource {
    pub fn to_db(self) -> &'static str {
        match self {
            DocumentContentSource::DbContent => "DB_CONTENT",
            DocumentContentSource::InternalAsset => "INTERNAL_ASSET",
            DocumentContentSource::ExternalUrl => "EXTERNAL_URL",
            DocumentContentSource::ReferenceOnly => "REFERENCE_ONLY",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "INTERNAL_ASSET" | "internal_asset" => DocumentContentSource::InternalAsset,
            "EXTERNAL_URL" | "external_url" => DocumentContentSource::ExternalUrl,
            "REFERENCE_ONLY" | "reference_only" => DocumentContentSource::ReferenceOnly,
            _ => DocumentContentSource::DbContent,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentContentFormat {
    PlainText,
    Markdown,
    Internal,
}

impl DocumentContentFormat {
    pub fn to_db(self) -> &'static str {
        match self {
            DocumentContentFormat::PlainText => "PLAIN_TEXT",
            DocumentContentFormat::Markdown => "MARKDOWN",
            DocumentContentFormat::Internal => "INTERNAL",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "MARKDOWN" | "markdown" => DocumentContentFormat::Markdown,
            "INTERNAL" | "internal" => DocumentContentFormat::Internal,
            _ => DocumentContentFormat::PlainText,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentStatus {
    Active,
    Archived,
}

impl DocumentStatus {
    pub fn to_db(self) -> &'static str {
        match self {
            DocumentStatus::Active => "ACTIVE",
            DocumentStatus::Archived => "ARCHIVED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "ARCHIVED" | "archived" => DocumentStatus::Archived,
            _ => DocumentStatus::Active,
        }
    }

    /// Whether a document in this status may back a published post.
    /// Per `doc/publication.md`: active documents allow published posts;
    /// archived documents give archived posts.
    pub fn permits_published_post(self) -> bool {
        matches!(self, DocumentStatus::Active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mirrors the document row of the table in doc/publication.md.
    #[test]
    fn permits_published_post_matches_publication_model() {
        assert!(DocumentStatus::Active.permits_published_post());
        assert!(!DocumentStatus::Archived.permits_published_post());
    }
}
