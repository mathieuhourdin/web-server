use crate::schema::*;
use diesel::prelude::*;
use super::model::*;
use crate::entities::{error::{PpdcError}, resource::*};
use crate::http::{HttpRequest, HttpResponse};

pub fn get_thought_inputs_for_user(user_id: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let user_id = HttpRequest::parse_uuid(user_id)?;
    let (offset, limit) = request.get_pagination();
    let thought_inputs = Interaction::load_paginated(
        offset,
        limit, 
        interactions::table
            .filter(interactions::interaction_type.eq("inpt"))
            .filter(interactions::interaction_user_id.eq(user_id))
            .filter(interactions::interaction_is_public.eq(true)).into_boxed(),
        "pbsh",
        "all"

    )?;
    HttpResponse::ok()
        .json(&thought_inputs)
}

pub fn get_thought_inputs(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let (offset, limit) = request.get_pagination();
    let thought_inputs = Interaction::load_paginated(
        offset,
        limit, 
        interactions::table
            .filter(interactions::interaction_type.eq("inpt"))
            .filter(interactions::interaction_is_public.eq(true)).into_boxed(),
        "pbsh",
        "all"
    )?;
    HttpResponse::ok()
        .json(&thought_inputs)
}

pub fn get_one_thought_input_route(id: &str, _request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let id = HttpRequest::parse_uuid(id)?;
    HttpResponse::ok()
        .json(&Interaction::find(id)?)
}

pub fn post_thought_input_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {

    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }
    let NewInteractionWithNewResource { mut interaction, mut resource } = serde_json::from_str::<NewInteractionWithNewResource>(&request.body[..])?;

    resource.maturing_state = Some("fnsh".to_string());
    resource.publishing_state = Some("pbsh".to_string());
    resource.is_external = Some(true);

    let resource = resource.create()?;

    interaction.interaction_user_id = request.session.as_ref().unwrap().user_id;
    interaction.interaction_type = Some("inpt".to_string());
    interaction.resource_id = Some(resource.id);
    let interaction = interaction.create()?;

    let thought_input_with_resource = InteractionWithResource { resource, interaction };
    HttpResponse::ok()
        .json(&thought_input_with_resource)
}

pub fn put_thought_input_route(id: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }
    let id = HttpRequest::parse_uuid(id)?;
    let db_thought_input = Interaction::find(id)?;
    let thought_input = serde_json::from_str::<NewInteraction>(&request.body[..])?;
    if db_thought_input.interaction_user_id != request.session.as_ref().unwrap().user_id.unwrap() 
    {
        return Ok(HttpResponse::unauthorized());
    }
    HttpResponse::ok()
        .json(&thought_input.update(&id)?)
}
