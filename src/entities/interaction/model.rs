use serde::{Serialize, Deserialize};
use diesel::prelude::*;
use uuid::Uuid;
use chrono::NaiveDateTime;
use crate::db;
use crate::schema::*;
use diesel;
use crate::entities::{error::{PpdcError}, user::User, resource::{Resource, NewResource}};

#[derive(Serialize, Deserialize, Clone, Queryable, Selectable, AsChangeset)]
#[diesel(table_name=interactions)]
pub struct Interaction {
    pub id: Uuid,
    pub interaction_user_id: Uuid,
    pub interaction_progress: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
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

#[derive(Deserialize)]
pub struct NewInteractionWithNewResource {
    #[serde(flatten)]
    pub interaction: NewInteraction,
    pub resource: NewResource
}

#[derive(Deserialize, Insertable, Queryable, AsChangeset)]
#[diesel(table_name=interactions)]
pub struct NewInteraction {
    pub interaction_user_id: Option<Uuid>,
    pub interaction_progress: i32,
    pub interaction_comment: Option<String>,
    pub interaction_date: Option<NaiveDateTime>,
    pub interaction_type: Option<String>,
    pub interaction_is_public: Option<bool>,
    pub resource_id: Option<Uuid>
}

impl Interaction {

    pub fn load_paginated(offset: i64, limit: i64, filtered_interactions_query: interactions::BoxedQuery<diesel::pg::Pg>, resource_publishing_state: &str, resource_type: &str) -> Result<Vec<InteractionWithResource>, PpdcError> {

        let mut conn = db::establish_connection();

        let mut query = filtered_interactions_query
            .offset(offset)
            .limit(limit)
            .inner_join(resources::table)
            .filter(resources::publishing_state.eq(resource_publishing_state));
        if resource_type != "all" {
            query = query
            .filter(resources::resource_type.eq(resource_type));
        }
        let thought_outputs = query
            .select((Interaction::as_select(), Resource::as_select()))
            .load::<(Interaction, Resource)>(&mut conn)?
            .into_iter()
            .map(|(interaction, resource)| InteractionWithResource { interaction, resource })
            .collect();
        Ok(thought_outputs)
    }

    pub fn filter_outputs() -> interactions::BoxedQuery<'static, diesel::pg::Pg> {
        let query = interactions::table.into_boxed();
        query
            .filter(interactions::interaction_type.eq("outp"))
    }

    pub fn find_paginated_outputs_published(offset: i64, limit: i64, resource_type: &str) -> Result<Vec<InteractionWithResource>, PpdcError> {

        Interaction::load_paginated(
            offset,
            limit,
            Interaction::filter_outputs(),
            "pbsh",
            resource_type
        )
    }
    pub fn find_paginated_outputs_drafts(offset: i64, limit: i64, user_id: Uuid, resource_type: &str) -> Result<Vec<InteractionWithResource>, PpdcError> {
        Interaction::load_paginated(
            offset,
            limit,
            Interaction::filter_outputs()
                .filter(interactions::interaction_user_id.eq(user_id)),
            "drft",
            resource_type
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
