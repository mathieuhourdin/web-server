use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::{ErrorType, PpdcError},
    interaction::model::Interaction,
    resource::{entity_type::EntityType, maturing_state::MaturingState, Resource},
};
use crate::schema::{interactions, resources};
use diesel::prelude::*;

use super::model::{Post, PostType};

impl Post {
    fn from_interaction_and_resource(interaction: Interaction, resource: Resource) -> Post {
        Post {
            id: interaction.id,
            resource_id: resource.id,
            title: resource.title,
            subtitle: resource.subtitle,
            content: resource.content,
            post_type: PostType::from_interaction_type(
                interaction.interaction_type.as_deref().unwrap_or("outp"),
            ),
            user_id: interaction.interaction_user_id,
            publishing_date: Some(interaction.interaction_date),
            publishing_state: resource.publishing_state,
            maturing_state: resource.maturing_state,
            created_at: resource.created_at,
            updated_at: resource.updated_at,
        }
    }

    pub fn to_resource(&self) -> Resource {
        Resource {
            id: self.resource_id,
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            content: self.content.clone(),
            external_content_url: None,
            comment: None,
            image_url: None,
            resource_type: self.post_type.into(),
            entity_type: EntityType::PublicPost,
            maturing_state: self.maturing_state,
            publishing_state: self.publishing_state.clone(),
            category_id: None,
            is_external: false,
            created_at: self.created_at,
            updated_at: self.updated_at,
            resource_subtype: None,
        }
    }

    fn ensure_public_post(resource: &Resource) -> Result<(), PpdcError> {
        if resource.is_public_post() {
            Ok(())
        } else {
            Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Resource is not a public post".to_string(),
            ))
        }
    }

    pub fn find(id: Uuid, pool: &DbPool) -> Result<Post, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let (interaction, resource) = interactions::table
            .inner_join(resources::table)
            .filter(interactions::id.eq(id))
            .filter(interactions::interaction_type.eq_any(vec!["outp", "inpt", "pblm", "wish"]))
            .filter(resources::entity_type.eq(EntityType::PublicPost))
            .select((Interaction::as_select(), Resource::as_select()))
            .first::<(Interaction, Resource)>(&mut conn)?;
        Self::ensure_public_post(&resource)?;
        Ok(Post::from_interaction_and_resource(interaction, resource))
    }

    pub fn find_full(id: Uuid, pool: &DbPool) -> Result<Post, PpdcError> {
        Post::find(id, pool)
    }

    pub fn find_for_user(
        user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<Vec<Post>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let interaction_resources = interactions::table
            .inner_join(resources::table)
            .filter(interactions::interaction_user_id.eq(user_id))
            .filter(interactions::interaction_type.eq_any(vec!["outp", "inpt", "pblm", "wish"]))
            .filter(resources::entity_type.eq(EntityType::PublicPost))
            .order(interactions::interaction_date.desc())
            .offset(offset)
            .limit(limit)
            .select((Interaction::as_select(), Resource::as_select()))
            .load::<(Interaction, Resource)>(&mut conn)?;

        let posts = interaction_resources
            .into_iter()
            .map(|(interaction, resource)| Post::from_interaction_and_resource(interaction, resource))
            .collect::<Vec<_>>();
        Ok(posts)
    }

    pub fn find_filtered(
        post_type: Option<PostType>,
        user_id: Option<Uuid>,
        maturing_state: Option<MaturingState>,
        limit: i64,
        pool: &DbPool,
    ) -> Result<Vec<Post>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let mut query = interactions::table
            .inner_join(resources::table)
            .filter(interactions::interaction_type.eq_any(vec!["outp", "inpt", "pblm", "wish"]))
            .filter(resources::entity_type.eq(EntityType::PublicPost))
            .into_boxed();

        if let Some(post_type) = post_type {
            query = query.filter(interactions::interaction_type.eq(post_type.to_interaction_type()));
        }
        if let Some(user_id) = user_id {
            query = query.filter(interactions::interaction_user_id.eq(user_id));
        }
        if let Some(maturing_state) = maturing_state {
            query = query.filter(resources::maturing_state.eq(maturing_state));
        }

        let interaction_resources = query
            .order(interactions::interaction_date.desc())
            .limit(limit.max(1))
            .select((Interaction::as_select(), Resource::as_select()))
            .load::<(Interaction, Resource)>(&mut conn)?;

        let posts = interaction_resources
            .into_iter()
            .map(|(interaction, resource)| Post::from_interaction_and_resource(interaction, resource))
            .collect::<Vec<_>>();
        Ok(posts)
    }
}
