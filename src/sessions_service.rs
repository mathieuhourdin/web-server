use crate::http::{HttpRequest, HttpResponse, StatusCode};
use std::collections::HashMap;

pub async fn post_session_route(request: &HttpRequest) -> HttpResponse { 
    HttpResponse::from(StatusCode::Ok, HashMap::new(), String::from("Ok"))
}
