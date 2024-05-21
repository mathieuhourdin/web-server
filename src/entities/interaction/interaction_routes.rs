use crate::http::{HttpRequest, HttpResponse};
use diesel::prelude::*;
use crate::schema::*;
use serde_json;
use super::model::*;
use crate::entities::{error::PpdcError};

pub fn post_interaction_for_resource(resource_id: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }

    let resource_id = HttpRequest::parse_uuid(resource_id)?;

    let mut interaction = serde_json::from_str::<NewInteraction>(&request.body[..])?;

    interaction.interaction_user_id = Some(interaction.interaction_user_id.unwrap_or(request.session.as_ref().unwrap().user_id.unwrap()));
    interaction.resource_id = Some(resource_id);
    interaction.interaction_type = Some(interaction.interaction_type.unwrap_or("outp".to_string()));
    HttpResponse::ok()
        .json(&interaction.create()?)
}

pub fn get_interactions(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let user_id = request.session.as_ref().unwrap().user_id;
    if user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }
    let interaction_type = request.query.get("interaction_type").map(|value| &value[..]).unwrap_or("inpt");
    let maturing_state = request.query.get("maturing_state").map(|value| &value[..]).unwrap_or("fnsh");
    //let mut maturing_state = "fnsh";
    let interactions_filtered;
    if interaction_type == "rvew" || interaction_type == "outp" && maturing_state == "idea" {
        interactions_filtered = interactions::table
            .filter(interactions::interaction_type.eq(interaction_type))
            .filter(interactions::interaction_is_public.eq(true))
            .filter(interactions::interaction_user_id.eq(user_id.unwrap())).into_boxed();
    } else {
        interactions_filtered = interactions::table
            .filter(interactions::interaction_type.eq(interaction_type))
            .filter(interactions::interaction_is_public.eq(true)).into_boxed();
    }
    let (offset, limit) = request.get_pagination();
    let interactions = Interaction::load_paginated(
        offset,
        limit, 
        interactions_filtered,
        "pbsh",
        maturing_state,
        "all"

    )?;
    HttpResponse::ok()
        .json(&interactions)

}
    
pub fn put_interaction_route(uuid: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {

    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }
    let uuid = &HttpRequest::parse_uuid(uuid)?;
    let thought_output = serde_json::from_str::<NewInteraction>(&request.body[..])?;
    HttpResponse::ok()
        .json(&thought_output.update(uuid)?)
}

pub fn get_interactions_for_resource_route(resource_id: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let resource_id = HttpRequest::parse_uuid(resource_id)?;
    let limit: i64 = request.query.get("limit").unwrap_or(&"20".to_string()).parse().unwrap();
    let offset: i64 = request.query.get("offset").unwrap_or(&"0".to_string()).parse().unwrap();
    let interactions = Interaction::load_paginated(
        offset,
        limit, 
        interactions::table
            .filter(interactions::resource_id.eq(resource_id))
            .filter(interactions::interaction_is_public.eq(true)).into_boxed(),
        "pbsh",
        "fnsh",
        "all"
    )?;
    HttpResponse::ok()
        .json(&interactions)
}
