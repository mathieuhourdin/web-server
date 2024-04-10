use crate::router::route_request;
use crate::entities::error::PpdcError;
use regex::Regex;
use crate::sessions_service;
use crate::http::{HttpRequest, HttpResponse, StatusCode, Cookie};
use crate::environment;

pub async fn cors_middleware(request: &mut HttpRequest) -> HttpResponse {
    let regex_string = environment::get_allow_origin();
    let regex = Regex::new(&regex_string).expect("cors should be a valid regex");
    let mut allow_origin_host = String::new();
    if let Some(origin) = request.headers.get("Origin") {
        println!("Request Origin : {}", origin);
        if regex.is_match(origin) {
            allow_origin_host = origin.to_string();
        }
    }
    if let Some(origin) = request.headers.get("origin") {
        println!("Request origin : {}", origin);
        if regex.is_match(origin) {
            allow_origin_host = origin.to_string();
        }
    }
    let mut response;
    if request.method == "OPTIONS" {
        response = HttpResponse::ok()
            .body("Ok".to_string())
            .header("Access-Control-Allow-Headers", "authorization, content-type".to_string())
    } else if request.method == "GET" && request.path == "/" {
        // Google health check
        println!("Google Health Check");
        response = HttpResponse::ok()
            .body("Ok".to_string());
    } else {
        response = decode_cookie_for_request_middleware(request).await;
    }
    response.headers.insert("Access-Control-Allow-Origin".to_string(), allow_origin_host);
    response.headers.insert("Access-Control-Allow-Credentials".to_string(), "true".to_string());
    response.headers.insert("Access-Control-Allow-Methods".to_string(), "GET, POST, PUT, OPTIONS, DELETE".to_string());
    response
}

async fn decode_cookie_for_request_middleware(request: &mut HttpRequest) -> HttpResponse {
    let request_cookies = request.headers.get("Cookie");
    println!("Request cookies : {:#?}", request_cookies);
    let formatted_cookie = Cookie::from(request_cookies);
    request.cookies = formatted_cookie;
    transform_error_to_response_middleware(request).await
}

async fn transform_error_to_response_middleware(request: &mut HttpRequest) -> HttpResponse {
    match add_session_to_request_middleware(request).await {
        Ok(value) => return value,
        Err(err) => return HttpResponse::new(StatusCode::InternalServerError, "".to_string()).json(&err).unwrap()
    }
}

async fn add_session_to_request_middleware(request: &mut HttpRequest) -> Result<HttpResponse, PpdcError> {
    let session_cookie = sessions_service::create_or_attach_session(request).await?;
    let mut response = route_request(request).await?;
    println!("Session cookie : {session_cookie}");
    response.set_header("Set-Cookie", session_cookie);
    Ok(response)
}
