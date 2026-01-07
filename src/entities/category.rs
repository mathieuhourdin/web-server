use crate::db;
use crate::entities::error::PpdcError;
use crate::http::{HttpRequest, HttpResponse};
use crate::schema::categories;
use chrono::NaiveDateTime;
use diesel;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::categories)]
pub struct Category {
    pub id: Uuid,
    pub display_name: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Insertable, AsChangeset)]
#[diesel(table_name = crate::schema::categories)]
pub struct NewCategory {
    pub display_name: String,
}
impl NewCategory {
    pub fn create(self) -> Result<Category, PpdcError> {
        let mut conn = db::establish_connection();

        let category = diesel::insert_into(categories::table)
            .values(&self)
            .get_result(&mut conn)?;
        Ok(category)
    }

    pub fn update(self, id: &Uuid) -> Result<Category, PpdcError> {
        let mut conn = db::establish_connection();

        let category = diesel::update(categories::table)
            .filter(categories::id.eq(id))
            .set(&self)
            .get_result(&mut conn)?;
        Ok(category)
    }
}

impl Category {
    pub fn find_paginated(offset: i64, limit: i64) -> Result<Vec<Category>, PpdcError> {
        let mut conn = db::establish_connection();

        let category = categories::table
            .offset(offset)
            .limit(limit)
            .load::<Category>(&mut conn)?;
        Ok(category)
    }
}

pub fn get_categories_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let limit: i64 = request
        .query
        .get("limit")
        .unwrap_or(&"20".to_string())
        .parse()
        .unwrap();
    let offset: i64 = request
        .query
        .get("offset")
        .unwrap_or(&"0".to_string())
        .parse()
        .unwrap();
    HttpResponse::ok().json(&Category::find_paginated(offset, limit)?)
}

pub fn post_category_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let category = serde_json::from_str::<NewCategory>(&request.body[..])?;
    HttpResponse::ok().json(&NewCategory::create(category)?)
}
