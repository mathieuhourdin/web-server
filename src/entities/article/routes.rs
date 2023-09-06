use crate::http::{HttpRequest, HttpResponse};
use serde_json;
use crate::entities::{error::{PpdcError, ErrorType}, user::User};
use super::model::*;

pub fn get_article_route(uuid: &str) -> Result<HttpResponse, PpdcError> {
    let uuid = HttpRequest::parse_uuid(uuid)?;
    HttpResponse::ok()
        .json(&Article::find(uuid)?)
}

pub fn get_articles_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let limit: i64 = request.query.get("limit").unwrap_or(&"20".to_string()).parse().unwrap();
    let offset: i64 = request.query.get("offset").unwrap_or(&"0".to_string()).parse().unwrap();
    let with_author = request.query.get("author").map(|value| &value[..]).unwrap_or("false");
    if with_author == "true" {
        let articles = Article::find_paginated_with_author(offset, limit)?;
        HttpResponse::ok()
            .json(&articles)
    } else {
        let articles = Article::find_paginated(offset, limit)?;
        HttpResponse::ok()
            .json(&articles)
    }
}


