use crate::db::DbPool;
use crate::entities::{error::PpdcError, resource::NewResource};

use super::model::Journal;

impl Journal {
    /// Updates the Journal in the database.
    pub fn update(self, pool: &DbPool) -> Result<Journal, PpdcError> {
        let resource = self.to_resource();
        let updated_resource = NewResource::from(resource).update(&self.id, pool)?;
        Ok(Journal {
            title: updated_resource.title,
            subtitle: updated_resource.subtitle,
            content: updated_resource.content,
            updated_at: updated_resource.updated_at,
            ..self
        })
    }
}
