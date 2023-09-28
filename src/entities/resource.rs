use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;
use uuid::Uuid;
use crate::db;
use crate::schema::*;
use diesel::prelude::*;
use crate::entities::{error::{PpdcError}, user::User};
use crate::http::{HttpRequest, HttpResponse};

#[derive(Serialize, Deserialize, Clone, Queryable, Selectable, AsChangeset)]
#[diesel(table_name=resources)]
pub struct Resource {
    id: Uuid,
    #[serde(rename = "resource_title")]
    title: String,
    #[serde(rename = "resource_subtitle")]
    subtitle: String,
    #[serde(rename = "resource_content")]
    content: String,
    #[serde(rename = "resource_type")]
    type: Option<String>,
    #[serde(rename = "resource_external_content_url")]
    external_content_url: Option<String>,
    #[serde(rename = "resource_image_url")]
    image_url: Option<String>,
    #[serde(rename = "resource_comment")]
    comment: Option<String>,
    #[serde(rename = "resource_category_id")]
    category_id: Option<String>,
    #[serde(rename = "resource_created_at")]
    created_at: NaiveDateTime,
    #[serde(rename = "resource_updated_at")]
    updated_at: NaiveDateTime,
}

#[derive(Deserialize, Insertable, Queryable, AsChangeset)]
#[diesel(table_name=resources)]
pub struct NewResource {
    #[serde(rename = "resource_title")]
    title: String,
    #[serde(rename = "resource_subtitle")]
    subtitle: Option<String>,
    #[serde(rename = "resource_content")]
    content: Option<String>,
    #[serde(rename = "resource_type")]
    type: Option<String>,
    #[serde(rename = "resource_external_content_url")]
    external_content_url: Option<String>,
    #[serde(rename = "resource_image_url")]
    image_url: Option<String>,
    #[serde(rename = "resource_comment")]
    comment: Option<String>,
    #[serde(rename = "resource_category_id")]
    category_id: Option<String>,
}
