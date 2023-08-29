use serde::{Serialize, Deserialize};
use diesel::prelude::*;
use std::time::{SystemTime};
use uuid::Uuid;
use crate::db;
use crate::schema::articles;
use diesel;
use crate::entities::error::PpdcError;

#[derive(Serialize, Deserialize, Clone, Queryable, Selectable, AsChangeset)]
#[diesel(table_name=articles)]
pub struct Article {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub content: String,
    pub potential_improvements: String,
    pub author_id: Option<Uuid>,
    pub progress: i32,
    pub maturing_state: String,
    pub parent_id: Option<Uuid>,
    pub gdoc_url: Option<String>,
    pub image_url: Option<String>,
    pub url_slug: Option<String>,
    created_at: SystemTime,
    updated_at: SystemTime,
}

#[derive(Deserialize, Insertable, Queryable, AsChangeset)]
#[diesel(table_name=articles)]
pub struct NewArticle {
    pub title: String,
    pub description: String,
    pub content: String,
    pub potential_improvements: String,
    pub author_id: Option<Uuid>,
    pub progress: i32,
    pub maturing_state: String,
    pub parent_id: Option<Uuid>,
    pub gdoc_url: Option<String>,
    pub image_url: Option<String>,
    pub url_slug: Option<String>,
}

impl Article {

    pub fn find_paginated(offset: i64, limit: i64) -> Result<Vec<Article>, PpdcError> {
        let mut conn = db::establish_connection();

        let articles = articles::table
            .offset(offset)
            .limit(limit)
            .load::<Article>(&mut conn)?;
        Ok(articles)
    }

    pub fn find(id: Uuid) -> Result<Article, PpdcError> {
        let mut conn = db::establish_connection();

        let article = articles::table
            .filter(articles::id.eq(id))
            .first(&mut conn)?;
        Ok(article)
    }

    pub fn create(article: NewArticle) -> Result<Article, PpdcError> {
        let mut conn = db::establish_connection();

        let article = diesel::insert_into(articles::table)
            .values(&article)
            .get_result(&mut conn)?;
        Ok(article)
    }
    
    pub fn update(id: &Uuid, article: NewArticle) -> Result<Article, PpdcError> {
        let mut conn = db::establish_connection();

        let article = diesel::update(articles::table)
            .filter(articles::id.eq(id))
            .set(&article)
            .get_result(&mut conn)?;
        Ok(article)
    }
}
