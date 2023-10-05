use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;
use uuid::Uuid;
use crate::db;
use crate::schema::*;
use diesel;
use diesel::prelude::*;
use crate::entities::{error::{PpdcError}, user::User};
use crate::http::{HttpRequest, HttpResponse};

#[derive(Serialize, Deserialize, Clone, Queryable, Selectable, AsChangeset)]
#[diesel(table_name=resources)]
pub struct Resource {
    pub id: Uuid,
    #[serde(rename = "resource_title")]
    pub title: String,
    #[serde(rename = "resource_subtitle")]
    pub subtitle: String,
    #[serde(rename = "resource_content")]
    pub content: String,
    #[serde(rename = "resource_external_content_url")]
    pub external_content_url: Option<String>,
    #[serde(rename = "resource_comment")]
    pub comment: Option<String>,
    #[serde(rename = "resource_image_url")]
    pub image_url: Option<String>,
    #[serde(rename = "resource_type")]
    pub resource_type: String,
    #[serde(rename = "resource_maturing_state")]
    pub maturing_state: String,
    #[serde(rename = "resource_publishing_state")]
    pub publishing_state: String,
    #[serde(rename = "resource_category_id")]
    pub category_id: Option<Uuid>,
    pub is_external: bool,
    #[serde(rename = "resource_created_at")]
    pub created_at: NaiveDateTime,
    #[serde(rename = "resource_updated_at")]
    pub updated_at: NaiveDateTime,
}

#[derive(Deserialize, Insertable, Queryable, AsChangeset)]
#[diesel(table_name=resources)]
pub struct NewResource {
    #[serde(rename = "resource_title")]
    pub title: String,
    #[serde(rename = "resource_subtitle")]
    pub subtitle: String,
    #[serde(rename = "resource_content")]
    pub content: String,
    #[serde(rename = "resource_external_content_url")]
    pub external_content_url: Option<String>,
    #[serde(rename = "resource_comment")]
    pub comment: Option<String>,
    #[serde(rename = "resource_image_url")]
    pub image_url: Option<String>,
    #[serde(rename = "resource_type")]
    pub resource_type: Option<String>,
    #[serde(rename = "resource_maturing_state")]
    pub maturing_state: Option<String>,
    #[serde(rename = "resource_publishing_state")]
    pub publishing_state: Option<String>,
    #[serde(rename = "resource_category_id")]
    pub category_id: Option<Uuid>,
    pub is_external: bool,
}

impl Resource {
    pub fn find(id: Uuid) -> Result<Resource, PpdcError> {
        let mut conn = db::establish_connection();

        let result = resources::table
            .filter(resources::id.eq(id))
            .first(&mut conn)?;
        Ok(result)
    }
}

impl NewResource {
    pub fn create(self) -> Result<Resource, PpdcError> {
        let mut conn = db::establish_connection();

        let result = diesel::insert_into(resources::table)
            .values(&self)
            .get_result(&mut conn)?;
        Ok(result)
    }
    
    pub fn update(self, id: &Uuid) -> Result<Resource, PpdcError> {
        let mut conn = db::establish_connection();

        let result = diesel::update(resources::table)
            .filter(resources::id.eq(id))
            .set(&self)
            .get_result(&mut conn)?;
        Ok(result)
    }
}
