use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    interaction::model::Interaction,
    interaction::model::NewInteraction,
    resource::{entity_type::EntityType, resource_type::ResourceType, NewResource},
};
use crate::schema::interactions;
use diesel::prelude::*;

use super::model::{NewPost, Post};

impl Post {
    pub fn update(self, pool: &DbPool) -> Result<Post, PpdcError> {
        let existing_interaction = Interaction::find(self.id, pool)?;
        let resource = self.to_resource();
        NewResource::from(resource).update(&self.resource_id, pool)?;

        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        diesel::update(interactions::table.filter(interactions::id.eq(self.id)))
            .set((
                interactions::interaction_type.eq(Some(self.post_type.to_interaction_type().to_string())),
                interactions::interaction_date.eq(
                    self.publishing_date
                        .unwrap_or(existing_interaction.interaction_date),
                ),
            ))
            .execute(&mut conn)?;

        Post::find_full(self.id, pool)
    }
}

impl NewPost {
    pub fn create(self, pool: &DbPool) -> Result<Post, PpdcError> {
        let resource_type: ResourceType = self.post_type.into();

        let mut new_resource = NewResource::new(
            self.title,
            self.subtitle,
            self.content,
            resource_type,
        );
        new_resource.entity_type = Some(EntityType::PublicPost);
        new_resource.maturing_state = Some(self.maturing_state);
        new_resource.publishing_state = Some(self.publishing_state);

        let resource = new_resource.create(pool)?;

        let mut interaction = NewInteraction::new(self.user_id, resource.id);
        interaction.interaction_type = Some(self.post_type.to_interaction_type().to_string());
        interaction.interaction_date = self.publishing_date;
        let interaction = interaction.create(pool)?;

        Post::find_full(interaction.id, pool)
    }
}
