use super::model::*;
use diesel::prelude::*;
use crate::schema::*;
use serde_json;
use crate::http::{HttpRequest, HttpResponse};
use crate::entities::{error::PpdcError, user::User, resource::NewResource};

pub fn get_thought_output_route(uuid: &str) -> Result<HttpResponse, PpdcError> {
    let uuid = HttpRequest::parse_uuid(uuid)?;
    HttpResponse::ok()
        .json(&Interaction::find(uuid)?)
}

pub fn get_thought_outputs_for_user(user_id: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let user_id = HttpRequest::parse_uuid(user_id)?;
    let (offset, limit) = request.get_pagination();
    let thought_inputs = Interaction::load_paginated(
        offset,
        limit, 
        interactions::table
            .filter(interactions::interaction_type.eq("outp"))
            .filter(interactions::interaction_user_id.eq(user_id)).into_boxed(),
        "pbsh",
        "atcl"

    )?;
    HttpResponse::ok()
        .json(&thought_inputs)
}

pub fn get_thought_outputs_route(request: &HttpRequest, filter: &str) -> Result<HttpResponse, PpdcError> {
    let limit: i64 = request.query.get("limit").unwrap_or(&"20".to_string()).parse().unwrap();
    let offset: i64 = request.query.get("offset").unwrap_or(&"0".to_string()).parse().unwrap();
    let with_author = request.query.get("author").map(|value| &value[..]).unwrap_or("false");
    let drafts = request.query.get("drafts").map(|value| &value[..]).unwrap_or("false");
    if with_author == "true" {
        let thought_outputs = Interaction::find_paginated_outputs_published(offset, limit, filter)?;
        let thought_outputs: Vec<InteractionWithAuthorAndResource> = thought_outputs
            .into_iter()
            .map(|interaction| InteractionWithAuthorAndResource {
                author: User::find(&interaction.interaction.interaction_user_id).ok(),
                interaction: interaction.interaction,
                resource: interaction.resource
            })
            .collect();
        HttpResponse::ok()
            .json(&thought_outputs)
    } else if drafts == "true" {
        if request.session.as_ref().unwrap().user_id.is_none() {
            return HttpResponse::ok().json::<Vec<String>>(&vec![]);
        }
        let user_id = request.session.as_ref().unwrap().user_id.unwrap();
        let thought_outputs = Interaction::find_paginated_outputs_drafts(offset, limit, user_id, filter)?;
        HttpResponse::ok()
            .json(&thought_outputs)
    } else {
        let thought_outputs = Interaction::find_paginated_outputs_published(offset, limit, filter)?;
        HttpResponse::ok()
            .json(&thought_outputs)
    }
}

pub fn post_thought_outputs_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {

    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }

    println!("post_thought_outputs_route");

    let NewInteractionWithNewResource { mut interaction, mut resource } = serde_json::from_str::<NewInteractionWithNewResource>(&request.body[..])?;

    resource.is_external = Some(false);
    let resource = resource.create()?;

    interaction.interaction_user_id = request.session.as_ref().unwrap().user_id;
    interaction.interaction_type = Some("outp".to_string());
    interaction.resource_id = Some(resource.id);
    let interaction = interaction.create()?;

    let interaction_with_resource = InteractionWithResource { resource, interaction };
    HttpResponse::ok()
        .json(&interaction_with_resource)
}

pub fn put_thought_output_route(uuid: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {

    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }
    let uuid = &HttpRequest::parse_uuid(uuid)?;
    let mut thought_output = serde_json::from_str::<NewInteraction>(&request.body[..])?;
    thought_output.interaction_user_id = request.session.as_ref().unwrap().user_id;
    HttpResponse::ok()
        .json(&thought_output.update(uuid)?)
}
