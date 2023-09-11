use crate::http::{HttpRequest, HttpResponse};
use serde_json;
use crate::entities::error::PpdcError;
use super::model::*;

pub fn get_thought_output_route(uuid: &str) -> Result<HttpResponse, PpdcError> {
    let uuid = HttpRequest::parse_uuid(uuid)?;
    HttpResponse::ok()
        .json(&ThoughtOutput::find(uuid)?)
}

pub fn get_thought_outputs_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let limit: i64 = request.query.get("limit").unwrap_or(&"20".to_string()).parse().unwrap();
    let offset: i64 = request.query.get("offset").unwrap_or(&"0".to_string()).parse().unwrap();
    let with_author = request.query.get("author").map(|value| &value[..]).unwrap_or("false");
    let drafts = request.query.get("drafts").map(|value| &value[..]).unwrap_or("false");
    if with_author == "true" {
        let thought_outputs = ThoughtOutput::find_paginated_published_with_author(offset, limit)?;
        HttpResponse::ok()
            .json(&thought_outputs)
    } else if drafts == "true" {
        if request.session.as_ref().unwrap().user_id.is_none() {
            return HttpResponse::ok().json::<Vec<String>>(&vec![]);
        }
        let user_id = request.session.as_ref().unwrap().user_id.unwrap();
        let thought_outputs = ThoughtOutput::find_paginated_drafts(offset, limit, user_id)?;
        HttpResponse::ok()
            .json(&thought_outputs)
    } else {
        let thought_outputs = ThoughtOutput::find_paginated_published(offset, limit)?;
        HttpResponse::ok()
            .json(&thought_outputs)
    }
}

pub fn post_thought_outputs_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {

    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }
    println!("post_thought_outputs_route passing security");
    let mut thought_output = serde_json::from_str::<NewThoughtOutput>(&request.body[..])?;
    println!("post_thought_outputs_route passing serde");
    thought_output.author_id = request.session.as_ref().unwrap().user_id;
    HttpResponse::ok()
        .json(&ThoughtOutput::create(thought_output)?)
}

pub fn put_thought_output_route(uuid: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {

    println!("Put_thought_output_route with uuid : {:#?}", uuid);

    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }
    let uuid = &HttpRequest::parse_uuid(uuid)?;
    let mut thought_output = serde_json::from_str::<NewThoughtOutput>(&request.body[..])?;
    thought_output.author_id = request.session.as_ref().unwrap().user_id;
    HttpResponse::ok()
        .json(&ThoughtOutput::update(uuid, thought_output)?)
}
