use std::net::{TcpStream};
use std::io::{prelude::*};
use std::thread;
use http::{HttpRequest, HttpResponse, StatusCode, Cookie};
use entities::{user::self, thought_output::routes as thought_output, error::PpdcError, comment};
use entities::thought_input;
use entities::thought_input_usage;
use entities::category;
use regex::Regex;
use std::time::{SystemTime, Duration};
use chrono::{NaiveDateTime, Utc};

pub mod threadpool;
pub mod entities;
pub mod http;
pub mod sessions_service;
pub mod environment;
pub mod db;
pub mod schema;
pub mod legacy;

pub async fn handle_connection(mut stream: TcpStream) {

    let current_time = Utc::now().naive_utc();
    println!("[handle_connection lib.rs] Current time : {:#?}", current_time);

    if let Err(err) = stream.set_read_timeout(Some(Duration::from_secs(30))) {
        println!("Error setting read timeout");
        return;
    }

    let mut encoded_vector = vec![];

    loop {
        let mut buffer = [0; 1024];

        thread::sleep(Duration::from_millis(10));

        let read_bytes = stream.read(&mut buffer);

        let read_bytes = match read_bytes {
            Err(err) => {
                println!("Error during request package reading : {:#?}", err);
                let response = HttpResponse::new(StatusCode::BadRequest, "Unable to decode request".to_string());
                stream.write_all(response.to_stream().as_bytes()).unwrap();
                return;
            },
            Ok(value) => value
        };

        encoded_vector.append(&mut buffer[..read_bytes].to_vec());


        if read_bytes < 1024 {
            break;
        }
    }
    let string_request = String::from_utf8(encoded_vector);

    let string_request = match string_request {
        Err(err) => {
            println!("Error during request utf8 decoding : {:#?}", err);
            let response = HttpResponse::new(StatusCode::BadRequest, "Unable to decode request".to_string());
            stream.write_all(response.to_stream().as_bytes()).unwrap();
            return;
        },
        Ok(value) => value
    };

    println!("Request is : {string_request}");

    let mut http_request = match HttpRequest::from(&string_request) {
        Ok(value) => value,
        Err(_) => {
            let response = HttpResponse::new(StatusCode::BadRequest, "invalid request".to_string());
            let response = response.to_stream();
            stream.write_all(response.as_bytes()).unwrap();
            return;
        },
    };


    let http_response: HttpResponse = cors_middleware(&mut http_request).await;

    let response = http_response.to_stream(); 
    stream.write_all(response.as_bytes()).unwrap();
}

async fn cors_middleware(request: &mut HttpRequest) -> HttpResponse {
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
        Err(err) => return HttpResponse::new(StatusCode::InternalServerError, err.message)
    }
}

async fn add_session_to_request_middleware(request: &mut HttpRequest) -> Result<HttpResponse, PpdcError> {
    let session_cookie = sessions_service::create_or_attach_session(request).await?;
    let mut response = route_request(request).await?;
    println!("Session cookie : {session_cookie}");
    response.set_header("Set-Cookie", session_cookie);
    Ok(response)
}

async fn route_request(request: &mut HttpRequest) -> Result<HttpResponse, PpdcError> {
    let session_id = &request.session.as_ref().unwrap().id;
    println!("Request with session id : {session_id}");
    match (&request.method[..], &request.parsed_path()[..]) {
        ("GET", [""]) => HttpResponse::from_file(StatusCode::Ok, "hello.html"),
        ("GET", ["mathilde"]) => HttpResponse::from_file(StatusCode::Ok, "mathilde.html"),
        ("POST", ["mathilde"]) => legacy::post_mathilde_route(&request),
        ("GET", ["users", id]) => user::get_user(id, &request),
        ("POST", ["users"]) => user::post_user(&request),
        ("GET", ["login"]) => HttpResponse::from_file(StatusCode::Ok, "login.html"),
        ("POST", ["sessions"]) => sessions_service::post_session_route(request).await,
        ("GET", ["sessions"]) => sessions_service::get_session_route(request).await,
        ("GET", ["thought_outputs", id, "comments"]) => comment::get_comments_for_thought_output(id, &request),
        ("POST", ["thought_outputs", id, "comments"]) => comment::post_comment_route(id, &request),
        ("PUT", ["comments", id]) => comment::put_comment(id, &request),
        ("GET", ["create-article"]) => HttpResponse::from_file(StatusCode::Ok, "create-article.html"),
        ("GET", ["list-article"]) => HttpResponse::from_file(StatusCode::Ok, "list-article.html"),
        ("GET", ["see-article", uuid]) => legacy::see_article(uuid, &request),
        ("GET", ["edit-article", uuid]) => legacy::edit_article(uuid, &request),
        ("GET", ["problems"]) => thought_output::get_thought_outputs_route(&request, "pblm"),
        ("GET", ["articles"]) => thought_output::get_thought_outputs_route(&request, "atcl"),
        ("GET", ["articles", id]) => thought_output::get_thought_output_route(id),
        ("POST", ["articles"]) => thought_output::post_thought_outputs_route(&request),
        ("GET", ["categories"]) => category::get_categories_route(&request),
        ("POST", ["categories"]) => category::post_category_route(&request),
        ("GET", ["thought_outputs"]) => thought_output::get_thought_outputs_route(&request, "all"),
        ("GET", ["thought_outputs", uuid]) => thought_output::get_thought_output_route(uuid),
        ("POST", ["thought_outputs"]) => thought_output::post_thought_outputs_route(&request),
        ("PUT", ["thought_outputs", uuid]) => thought_output::put_thought_output_route(uuid, &request),
        ("GET", ["thought_outputs", id, "thought_input_usages"]) => thought_input_usage::get_thought_input_usages_for_thought_output_route(id, &request),
        ("POST", ["thought_input_usages"]) => thought_input_usage::post_thought_input_usage_route(&request),
        ("POST", ["thought_inputs"]) => thought_input::post_thought_input_route(&request),
        ("PUT", ["thought_inputs", id]) => thought_input::put_thought_input_route(id, &request),
        ("GET", ["thought_inputs", id]) => thought_input::get_one_thought_input_route(id, &request),
        ("GET", ["thought_inputs"]) => thought_input::get_thought_inputs(&request),
        ("GET", ["users", id, "thought_inputs"]) => thought_input::get_thought_inputs_for_user(id, &request),
        ("GET", ["public", file_name]) => HttpResponse::from_file(StatusCode::Ok, file_name),
        _ => HttpResponse::from_file(StatusCode::NotFound, "404.html")
    }
}
