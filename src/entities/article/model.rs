use serde::{Serialize, Deserialize};
use diesel::prelude::*;
use uuid::Uuid;
use chrono::NaiveDateTime;
use crate::db;
use crate::schema::*;
use diesel;
use diesel::dsl::not;
use crate::entities::{error::{PpdcError}, user::User};

#[derive(Serialize, Deserialize, Clone, Queryable, Selectable, AsChangeset)]
#[diesel(table_name=thought_outputs)]
pub struct ThoughtOutput {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub content: String,
    pub potential_improvements: String,
    pub author_id: Option<Uuid>,
    pub progress: i32,
    pub maturing_state: String,
    pub publishing_state: String,
    pub parent_id: Option<Uuid>,
    pub gdoc_url: Option<String>,
    pub image_url: Option<String>,
    pub url_slug: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Queryable)]
pub struct ThoughtOutputWithAuthor {
    #[serde(flatten)]
    pub thought_output: ThoughtOutput,
    pub author: User,
}

#[derive(Deserialize, Insertable, Queryable, AsChangeset)]
#[diesel(table_name=thought_outputs)]
pub struct NewThoughtOutput {
    pub title: String,
    pub description: String,
    pub content: String,
    pub potential_improvements: String,
    pub author_id: Option<Uuid>,
    pub progress: i32,
    pub maturing_state: String,
    pub publishing_state: Option<String>,
    pub parent_id: Option<Uuid>,
    pub gdoc_url: Option<String>,
    pub image_url: Option<String>,
    pub url_slug: Option<String>,
}

impl ThoughtOutput {

    pub fn find_paginated_published(offset: i64, limit: i64) -> Result<Vec<ThoughtOutput>, PpdcError> {
        let mut conn = db::establish_connection();

        let thought_outputs = thought_outputs::table
            .filter(not(thought_outputs::publishing_state.eq("drft")))
            .offset(offset)
            .limit(limit)
            .load::<ThoughtOutput>(&mut conn)?;
        Ok(thought_outputs)
    }

    pub fn find_paginated_drafts(offset: i64, limit: i64, user_id: Uuid) -> Result<Vec<ThoughtOutput>, PpdcError> {
        let mut conn = db::establish_connection();

        let thought_outputs = thought_outputs::table
            .filter(thought_outputs::publishing_state.eq("drft"))
            .filter(thought_outputs::author_id.eq(user_id))
            .offset(offset)
            .limit(limit)
            .load::<ThoughtOutput>(&mut conn)?;
        Ok(thought_outputs)
    }

    pub fn find_paginated_published_with_author(offset: i64, limit: i64) -> Result<Vec<ThoughtOutputWithAuthor>, PpdcError> {
        let mut conn = db::establish_connection();

        let thought_outputs_with_author = thought_outputs::table
            .filter(not(thought_outputs::publishing_state.eq("drft")))
            .inner_join(users::table)
            .offset(offset)
            .limit(limit)
            .select((ThoughtOutput::as_select(), User::as_select()))
            .load::<(ThoughtOutput, User)>(&mut conn)?
            .into_iter()
            .map(|(thought_output, author)| ThoughtOutputWithAuthor { thought_output, author })
            .collect();
        Ok(thought_outputs_with_author)
    }

    pub fn find(id: Uuid) -> Result<ThoughtOutput, PpdcError> {
        let mut conn = db::establish_connection();

        let thought_output = thought_outputs::table
            .filter(thought_outputs::id.eq(id))
            .first(&mut conn)?;
        Ok(thought_output)
    }

    pub fn create(thought_output: NewThoughtOutput) -> Result<ThoughtOutput, PpdcError> {
        let mut conn = db::establish_connection();

        let thought_output = diesel::insert_into(thought_outputs::table)
            .values(&thought_output)
            .get_result(&mut conn)?;
        Ok(thought_output)
    }
    
    pub fn update(id: &Uuid, thought_output: NewThoughtOutput) -> Result<ThoughtOutput, PpdcError> {
        let mut conn = db::establish_connection();

        let thought_output = diesel::update(thought_outputs::table)
            .filter(thought_outputs::id.eq(id))
            .set(&thought_output)
            .get_result(&mut conn)?;
        Ok(thought_output)
    }
}
