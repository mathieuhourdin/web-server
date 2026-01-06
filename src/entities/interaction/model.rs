use serde::{Serialize, Deserialize};
use diesel::prelude::*;
use uuid::Uuid;
use chrono::{NaiveDateTime, Utc, NaiveDate};
use crate::schema::*;
use diesel;
use crate::entities::{error::{PpdcError}, user::User, resource::{Resource, NewResource}};
use crate::db::DbPool;
use diesel::debug_query;
use diesel::pg::Pg;

//TODO integrate in interaction
pub enum InteractionType {
    Output,
    Input,
    Review,
    Auto,
    Analysis
}

impl InteractionType {
    pub fn to_string(self) -> String {
        match self {
            InteractionType::Output => "outp",
            InteractionType::Input => "inpt",
            InteractionType::Review => "rvew",
            InteractionType::Auto => "auto",
            InteractionType::Analysis => "anly",
        }.to_string()
    }
}

#[derive(Serialize, Deserialize, Clone, Queryable, Selectable, AsChangeset, Debug)]
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

#[derive(Serialize, Deserialize, Queryable, Debug)]
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

    pub fn load_paginated(
        offset: i64,
        limit: i64,
        filtered_interactions_query: interactions::BoxedQuery<diesel::pg::Pg>,
        resource_maturing_state: &str,
        resource_type: &str,
        pool: &DbPool
    ) -> Result<Vec<InteractionWithResource>, PpdcError> {

        let mut conn = pool.get().expect("Failed to get a connection from the pool");

        let mut query = filtered_interactions_query
            .offset(offset)
            .limit(limit)
            .inner_join(resources::table);
        if resource_maturing_state != "all" {
            query = query
                .filter(resources::maturing_state.eq(resource_maturing_state));
        }
        if resource_type != "all" {
            query = query
            .filter(resources::resource_type.eq(resource_type));
        }
        println!("{}", debug_query::<Pg, _>(&query));
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

    pub fn find_paginated_outputs_published(
        offset: i64,
        limit: i64,
        resource_type: &str,
        pool: &DbPool
    ) -> Result<Vec<InteractionWithResource>, PpdcError> {

        Interaction::load_paginated(
            offset,
            limit,
            Interaction::filter_outputs(),
            "fnsh",
            resource_type,
            pool
        )
    }
    pub fn find_paginated_outputs_drafts(
        offset: i64,
        limit: i64,
        user_id: Uuid,
        resource_type: &str,
        pool: &DbPool
    ) -> Result<Vec<InteractionWithResource>, PpdcError> {
        Interaction::load_paginated(
            offset,
            limit,
            Interaction::filter_outputs()
                .filter(interactions::interaction_user_id.eq(user_id)),
            "drft",
            resource_type,
            pool
            )
    }

    pub fn find_all_draft_traces_for_user_before_date(user_id: Uuid, date: NaiveDate, pool: &DbPool) -> Result<Vec<InteractionWithResource>, PpdcError> {
        Interaction::load_paginated(
            0,
            200,
            Interaction::filter_outputs()
                .filter(interactions::interaction_user_id.eq(user_id))
                .filter(interactions::interaction_date.lt(date.and_hms_opt(12, 0, 0).unwrap())),
            "drft",
            "trce",
            pool
        )
    }

    pub fn find_last_analysis_for_user(user_id: Uuid, pool: &DbPool) -> Result<Option<InteractionWithResource>, PpdcError> {
        let interactions = Interaction::load_paginated(
            0,
            1,
            interactions::table
                .into_boxed()
                .filter(interactions::interaction_user_id.eq(user_id))
                .filter(interactions::interaction_type.eq("anly"))
                .order(interactions::interaction_date.desc()),
            "fnsh",
            "anly",
            pool
        )?;
        Ok(interactions.into_iter().next())
    }

    pub fn find_paginated_outputs_problems(
        offset: i64,
        limit: i64,
        user_id: Uuid,
        pool: &DbPool
    ) -> Result<Vec<InteractionWithResource>, PpdcError> {
        Interaction::load_paginated(
            offset,
            limit,
            Interaction::filter_outputs()
                .filter(interactions::interaction_user_id.eq(user_id)),
            "fnsh",
            "pblm",
            pool
        )
    }

    pub fn find(id: Uuid, pool: &DbPool) -> Result<Interaction, PpdcError> {
        let mut conn = pool.get().expect("Failed to get a connection from the pool");

        let thought_output = interactions::table
            .filter(interactions::id.eq(id))
            .first(&mut conn)?;
        Ok(thought_output)
    }

    pub fn find_user_journal(user_id: Uuid, journal_id: Uuid, pool: &DbPool) -> Result<Interaction, PpdcError> {
        let mut conn = pool.get().expect("Failed to get a connection from the pool");
        let (journal_interaction, _resource) : (Interaction, Resource) = interactions::table
            .filter(interactions::interaction_type.eq("outp"))
            .filter(interactions::interaction_user_id.eq(user_id))
            .inner_join(resources::table)
            .filter(resources::resource_type.eq("jrnl"))
            .filter(resources::id.eq(journal_id))                                               
            .first::<(Interaction, Resource)>(&mut conn)?;
        Ok(journal_interaction)
    }
}

impl NewInteraction {
    pub fn new(user_id: Uuid, resource_id: Uuid) -> Self {
        Self {
            interaction_user_id: Some(user_id),
            interaction_progress: 100,
            interaction_comment: None,
            interaction_date: Some(Utc::now().naive_utc()),
            interaction_type: Some("outp".to_string()),
            interaction_is_public: Some(true),
            resource_id: Some(resource_id)
        }
    }

    pub fn create(self, pool: &DbPool) -> Result<Interaction, PpdcError> {
        let mut conn = pool.get().expect("Failed to get a connection from the pool");

        let result = diesel::insert_into(interactions::table)
            .values(&self)
            .get_result(&mut conn)?;
        Ok(result)
    }
    
    pub fn update(self, id: &Uuid, pool: &DbPool) -> Result<Interaction, PpdcError> {
        let mut conn = pool.get().expect("Failed to get a connection from the pool");

        let result = diesel::update(interactions::table)
            .filter(interactions::id.eq(id))
            .set(&self)
            .get_result(&mut conn)?;
        Ok(result)
    }

    pub fn create_instant(user_id: Uuid, resource_id: Uuid, pool: &DbPool) -> Result<Interaction, PpdcError> {
        let new_resource = Self {
            interaction_user_id: Some(user_id),
            interaction_progress: 100,
            interaction_comment: None,
            interaction_date: Some(Utc::now().naive_utc()),
            interaction_type: Some("outp".to_string()),
            interaction_is_public: Some(true),
            resource_id: Some(resource_id)
        };
        new_resource.create(pool)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_paginated_outputs_problems() {
        use crate::environment::get_database_url;
        let database_url = get_database_url();
        let manager = diesel::r2d2::ConnectionManager::new(database_url);
        let pool = DbPool::new(manager).expect("Failed to create connection pool");
        let string_uuid = "2d01ac77-eeec-4b47-bf8c-c58862f57b63";
        let uuid = Uuid::parse_str(string_uuid).unwrap();
        let interactions = Interaction::find_paginated_outputs_problems(0, 10, uuid, &pool).unwrap();
        println!("Interactions: {:?}", interactions);
        assert_eq!(interactions.len(), 0);
    }
}
