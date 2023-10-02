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
    pub resource_title: String,
    pub resource_subtitle: String,
    pub resource_content: String,
    pub resource_comment: String,
    pub interaction_user_id: Option<Uuid>,
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
    pub resource_title: String,
    pub resource_subtitle: String,
    pub resource_content: String,
    pub resource_comment: String,
    pub interaction_user_id: Option<Uuid>,
    pub interaction_progress: i32,
    pub resource_maturing_state: String,
    pub resource_publishing_state: String,
    pub resource_parent_id: Option<Uuid>,
    pub resource_external_content_url: Option<String>,
    pub resource_image_url: Option<String>,
    pub resource_type: String,
    pub resource_category_id: Option<Uuid>,
    pub interaction_comment: Option<String>,
    pub interaction_date: Option<NaiveDateTime>,
    pub interaction_type: Option<String>,
    pub interaction_is_public: Option<bool>,
}

impl ThoughtOutput {

    pub fn find_paginated_published(offset: i64, limit: i64, resource_type: &str) -> Result<Vec<ThoughtOutput>, PpdcError> {
        let mut conn = db::establish_connection();

        let mut query = thought_outputs::table.into_boxed();
        if resource_type != "all" {
            query = query.filter(thought_outputs::resource_type.eq(resource_type))
        };
        let thought_outputs = query
            .filter(thought_outputs::interaction_type.eq("outp"))
            .filter(thought_outputs::resource_publishing_state.ne("drft"))
            .offset(offset)
            .limit(limit)
            .load::<ThoughtOutput>(&mut conn)?;
        Ok(thought_outputs)
    }

    pub fn find_paginated_drafts(offset: i64, limit: i64, user_id: Uuid, resource_type: &str) -> Result<Vec<ThoughtOutput>, PpdcError> {
        let mut conn = db::establish_connection();

        let mut query = thought_outputs::table.into_boxed();
        if resource_type != "all" {
            query = query.filter(thought_outputs::resource_type.eq(resource_type))
        };
        let thought_outputs = query
            .filter(thought_outputs::interaction_type.eq("outp"))
            .filter(thought_outputs::resource_publishing_state.eq("drft"))
            .filter(thought_outputs::interaction_user_id.eq(user_id))
            .offset(offset)
            .limit(limit)
            .load::<ThoughtOutput>(&mut conn)?;
        Ok(thought_outputs)
    }

    pub fn find_paginated_published_with_author(offset: i64, limit: i64, resource_type: &str) -> Result<Vec<ThoughtOutputWithAuthor>, PpdcError> {
        let mut conn = db::establish_connection();

        let mut query = thought_outputs::table.select(thought_outputs::all_columns).into_boxed();
        if resource_type != "all" {
            query = query.filter(thought_outputs::resource_type.eq(resource_type))
        };
        let thought_outputs_with_author = query
            .filter(thought_outputs::interaction_type.eq("outp"))
            .filter(thought_outputs::resource_publishing_state.ne("drft"))
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
