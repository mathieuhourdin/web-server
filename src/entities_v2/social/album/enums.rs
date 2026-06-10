use serde::de::Deserializer;
use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumOrderingMode {
    Chronological,
    Manual,
    AddedAt,
}

impl AlbumOrderingMode {
    pub fn to_code(self) -> &'static str {
        match self {
            AlbumOrderingMode::Chronological => "chronological",
            AlbumOrderingMode::Manual => "manual",
            AlbumOrderingMode::AddedAt => "added_at",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "manual" | "MANUAL" | "Manual" => AlbumOrderingMode::Manual,
            "added_at" | "ADDED_AT" | "AddedAt" => AlbumOrderingMode::AddedAt,
            _ => AlbumOrderingMode::Chronological,
        }
    }

    pub fn to_db(self) -> &'static str {
        match self {
            AlbumOrderingMode::Chronological => "CHRONOLOGICAL",
            AlbumOrderingMode::Manual => "MANUAL",
            AlbumOrderingMode::AddedAt => "ADDED_AT",
        }
    }

    pub fn from_db(value: &str) -> Self {
        Self::from_code(value)
    }
}

impl Serialize for AlbumOrderingMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_code())
    }
}

impl<'de> Deserialize<'de> for AlbumOrderingMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(Self::from_code(&value))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumCompletionStatus {
    InProgress,
    Complete,
    Archived,
}

impl AlbumCompletionStatus {
    pub fn to_code(self) -> &'static str {
        match self {
            AlbumCompletionStatus::InProgress => "in_progress",
            AlbumCompletionStatus::Complete => "complete",
            AlbumCompletionStatus::Archived => "archived",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "complete" | "COMPLETE" | "Complete" => AlbumCompletionStatus::Complete,
            "archived" | "ARCHIVED" | "Archived" => AlbumCompletionStatus::Archived,
            _ => AlbumCompletionStatus::InProgress,
        }
    }

    pub fn to_db(self) -> &'static str {
        match self {
            AlbumCompletionStatus::InProgress => "IN_PROGRESS",
            AlbumCompletionStatus::Complete => "COMPLETE",
            AlbumCompletionStatus::Archived => "ARCHIVED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        Self::from_code(value)
    }

    /// Whether an album in this completion status may back a published post.
    /// Per `doc/publication.md`: in-progress and complete albums allow published
    /// posts; archived albums give archived posts.
    pub fn permits_published_post(self) -> bool {
        !matches!(self, AlbumCompletionStatus::Archived)
    }
}

impl Serialize for AlbumCompletionStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_code())
    }
}

impl<'de> Deserialize<'de> for AlbumCompletionStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(Self::from_code(&value))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumVisibility {
    Private,
    Published,
}

impl AlbumVisibility {
    pub fn to_code(self) -> &'static str {
        match self {
            AlbumVisibility::Private => "private",
            AlbumVisibility::Published => "published",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "published" | "PUBLISHED" | "Published" => AlbumVisibility::Published,
            _ => AlbumVisibility::Private,
        }
    }

    pub fn to_db(self) -> &'static str {
        match self {
            AlbumVisibility::Private => "PRIVATE",
            AlbumVisibility::Published => "PUBLISHED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        Self::from_code(value)
    }
}

impl Serialize for AlbumVisibility {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_code())
    }
}

impl<'de> Deserialize<'de> for AlbumVisibility {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(Self::from_code(&value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mirrors the album row of the table in doc/publication.md.
    #[test]
    fn permits_published_post_matches_publication_model() {
        assert!(AlbumCompletionStatus::InProgress.permits_published_post());
        assert!(AlbumCompletionStatus::Complete.permits_published_post());
        assert!(!AlbumCompletionStatus::Archived.permits_published_post());
    }
}
