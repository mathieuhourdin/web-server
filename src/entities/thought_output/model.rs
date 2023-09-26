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
    #[serde(rename = "title")]
    pub resource_title: String,
    #[serde(rename = "description")]
    pub resource_subtitle: String,
    #[serde(rename = "content")]
    pub resource_content: String,
    #[serde(rename = "potential_improvements")]
    pub resource_comment: String,
    pub author_id: Option<Uuid>,
    pub progress: i32,
    pub maturing_state: String,
    pub publishing_state: String,
    pub parent_id: Option<Uuid>,
    #[serde(rename = "gdoc_url ")]
    pub resource_external_content_url: Option<String>,
    #[serde(rename = "image_url")]
    pub resource_image_url: Option<String>,
    pub url_slug: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    #[serde(rename = "output_type")]
    pub resource_type: String,
    #[serde(rename = "category_id")]
    pub resource_category_id: Option<Uuid>
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
    #[serde(rename = "title")]
    pub resource_title: String,
    #[serde(rename = "description")]
    pub resource_subtitle: String,
    #[serde(rename = "content")]
    pub resource_content: String,
    #[serde(rename = "potential_improvements")]
    pub resource_comment: String,
    pub author_id: Option<Uuid>,
    pub progress: i32,
    pub maturing_state: String,
    pub publishing_state: Option<String>,
    pub parent_id: Option<Uuid>,
    #[serde(rename = "gdoc_url ")]
    pub resource_external_content_url: Option<String>,
    #[serde(rename = "image_url")]
    pub resource_image_url: Option<String>,
    pub url_slug: Option<String>,
    #[serde(rename = "output_type")]
    pub resource_type: String,
    #[serde(rename = "category_id")]
    pub resource_category_id: Option<Uuid>
}

impl ThoughtOutput {

    pub fn find_paginated_published(offset: i64, limit: i64, resource_type: &str) -> Result<Vec<ThoughtOutput>, PpdcError> {
        let mut conn = db::establish_connection();

        let mut query = thought_outputs::table.into_boxed();
        if resource_type != "all" {
            query = query.filter(thought_outputs::resource_type.eq(resource_type))
        };
        let thought_outputs = query.filter(not(thought_outputs::publishing_state.eq("drft")))
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
        let thought_outputs = query.filter(thought_outputs::publishing_state.eq("drft"))
            .filter(thought_outputs::author_id.eq(user_id))
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
        let thought_outputs_with_author = query.filter(not(thought_outputs::publishing_state.eq("drft")))
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
