use serde::{Serialize, Deserialize};
use diesel::prelude::*;
use uuid::Uuid;
use chrono::NaiveDateTime;
use crate::db;
use crate::schema::*;
use diesel;
use crate::entities::{error::{PpdcError}, user::User, resource::Resource};

#[derive(Serialize, Deserialize, Clone, Queryable, Selectable, AsChangeset)]
#[diesel(table_name=interactions)]
pub struct Interaction {
    pub id: Uuid,
    pub resource_title: String,
    pub resource_subtitle: String,
    pub resource_content: String,
    pub resource_comment: String,
    pub interaction_user_id: Uuid,
    pub interaction_progress: i32,
    pub resource_maturing_state: String,
    pub resource_publishing_state: String,
    pub resource_parent_id: Option<Uuid>,
    pub resource_external_content_url: Option<String>,
    pub resource_image_url: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub resource_type: String,
    pub resource_category_id: Option<Uuid>,
    pub interaction_comment: Option<String>,
    pub interaction_date: Option<NaiveDateTime>,
    pub interaction_type: Option<String>,
    pub interaction_is_public: bool,
    pub resource_id: Option<Uuid>,
}

#[derive(Serialize, Deserialize, Queryable)]
pub struct InteractionWithResource {
    #[serde(flatten)]
    pub interaction: Interaction,
    pub resource: Resource,
}

#[derive(Serialize, Deserialize, Queryable)]
pub struct InteractionWithAuthorAndResource {
    #[serde(flatten)]
    pub interaction: Interaction,
    pub author: Option<User>,
    #[serde(flatten)]
    pub resource: Resource,
}

#[derive(Serialize, Queryable)]
pub struct InteractionWithAuthor {
    pub interaction: Interaction,
    pub author: Option<User>,
}

#[derive(Deserialize, Insertable, Queryable, AsChangeset)]
#[diesel(table_name=interactions)]
pub struct NewInteraction {
    pub resource_title: String,
    pub resource_subtitle: Option<String>,
    pub resource_content: Option<String>,
    pub resource_comment: Option<String>,
    pub interaction_user_id: Option<Uuid>,
    pub interaction_progress: i32,
    pub resource_maturing_state: Option<String>,
    pub resource_publishing_state: Option<String>,
    pub resource_parent_id: Option<Uuid>,
    pub resource_external_content_url: Option<String>,
    pub resource_image_url: Option<String>,
    pub resource_type: String,
    pub resource_category_id: Option<Uuid>,
    pub interaction_comment: Option<String>,
    pub interaction_date: Option<NaiveDateTime>,
    pub interaction_type: Option<String>,
    pub interaction_is_public: Option<bool>,
    pub resource_id: Option<Uuid>
}

impl Interaction {

    pub fn load_paginated(offset: i64, limit: i64, filtered_interactions_query: interactions::BoxedQuery<diesel::pg::Pg>) -> Result<Vec<InteractionWithResource>, PpdcError> {

        let mut conn = db::establish_connection();

        let thought_outputs = filtered_interactions_query
            .offset(offset)
            .limit(limit)
            .inner_join(resources::table)
            .select((Interaction::as_select(), Resource::as_select()))
            .load::<(Interaction, Resource)>(&mut conn)?
            .into_iter()
            .map(|(interaction, resource)| InteractionWithResource { interaction, resource })
            .collect();
        Ok(thought_outputs)
    }

    pub fn filter_outputs(resource_type: &str) -> interactions::BoxedQuery<diesel::pg::Pg> {
        let mut query = interactions::table.into_boxed();
        if resource_type != "all" {
            query = query.filter(interactions::resource_type.eq(resource_type))
        };
        query
            .filter(interactions::interaction_type.eq("outp"))
    }

    pub fn find_paginated_outputs_published(offset: i64, limit: i64, resource_type: &str) -> Result<Vec<InteractionWithResource>, PpdcError> {

        Interaction::load_paginated(
            offset,
            limit,
            Interaction::filter_outputs(resource_type)
                .filter(interactions::resource_publishing_state.ne("drft")))
    }
    pub fn find_paginated_outputs_drafts(offset: i64, limit: i64, user_id: Uuid, resource_type: &str) -> Result<Vec<InteractionWithResource>, PpdcError> {
        Interaction::load_paginated(
            offset,
            limit,
            Interaction::filter_outputs(resource_type)
                .filter(interactions::resource_publishing_state.eq("drft"))
                .filter(interactions::interaction_user_id.eq(user_id))
                )
    }

    pub fn find(id: Uuid) -> Result<Interaction, PpdcError> {
        let mut conn = db::establish_connection();

        let thought_output = interactions::table
            .filter(interactions::id.eq(id))
            .first(&mut conn)?;
        Ok(thought_output)
    }

}

impl NewInteraction {
    pub fn create(self) -> Result<Interaction, PpdcError> {
        let mut conn = db::establish_connection();

        let result = diesel::insert_into(interactions::table)
            .values(&self)
            .get_result(&mut conn)?;
        Ok(result)
    }
    
    pub fn update(self, id: &Uuid) -> Result<Interaction, PpdcError> {
        let mut conn = db::establish_connection();

        let result = diesel::update(interactions::table)
            .filter(interactions::id.eq(id))
            .set(&self)
            .get_result(&mut conn)?;
        Ok(result)
    }
}
