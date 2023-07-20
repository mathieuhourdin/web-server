use crate::http::{HttpRequest, HttpResponse, StatusCode};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use serde_json;
use crate::database;

#[derive(Serialize, Deserialize)]
struct LoginCheck {
    username: String,
    password: String,
}

pub async fn post_session_route(request: &HttpRequest) -> HttpResponse { 
    let login_check = match serde_json::from_str::<LoginCheck>(&request.body[..]) {
        Err(err) => return HttpResponse::from(StatusCode::BadRequest, HashMap::new(), format!("Invalid post session : {:#?}", err)),
        Ok(value) => value,
    };
    let matching_user = match database::find_user_by_username(&login_check.username).await {
        Err(err) => return HttpResponse::from(
            StatusCode::InternalServerError,
            HashMap::new(),
            format!("Error : {:#?}", err)),
        Ok(value) => value,
    };
    let existing_user = match matching_user {
        Some(value) => value,
        None => return HttpResponse::from(
            StatusCode::NotFound,
            HashMap::new(),
            String::from("Can't find user with this username")),
    };
    println!("Matching user : {:#?}", existing_user.email);
    let is_valid_password = match existing_user.verify_password(&login_check.password.as_bytes()) {
        Ok(value) => value,
        Err(err) => return HttpResponse::from(
            StatusCode::InternalServerError,
            HashMap::new(),
            err.message,
        ),
    };
    if is_valid_password {
        return HttpResponse::from(StatusCode::Ok, HashMap::new(), String::from("Session is valid"));
    } else {
        return HttpResponse::from(StatusCode::BadRequest, HashMap::new(), String::from("Invalid password"));
    }
}
