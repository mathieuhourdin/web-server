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

pub fn post_articles_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {

    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }
    println!("post_articles_route passing security");
    let mut article = serde_json::from_str::<NewArticle>(&request.body[..])?;
    println!("post_articles_route passing serde");
    article.author_id = request.session.as_ref().unwrap().user_id;
    HttpResponse::ok()
        .json(&Article::create(article)?)
}

pub fn put_article_route(uuid: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {

    println!("Put_article_route with uuid : {:#?}", uuid);

    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }
    let uuid = &HttpRequest::parse_uuid(uuid)?;
    let mut article = serde_json::from_str::<NewArticle>(&request.body[..])?;
    article.author_id = request.session.as_ref().unwrap().user_id;
    HttpResponse::ok()
        .json(&Article::update(uuid, article)?)
}
