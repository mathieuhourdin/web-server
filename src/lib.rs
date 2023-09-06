use std::net::{TcpStream};
use std::io::{prelude::*};
use std::thread;
use std::time::Duration;
use http::{HttpRequest, HttpResponse, StatusCode, Cookie};
use entities::{user::self, article::routes as article, error::PpdcError, comment};
use regex::Regex;

pub mod threadpool;
pub mod entities;
pub mod http;
pub mod sessions_service;
pub mod environment;
pub mod db;
pub mod schema;
pub mod legacy;

pub async fn handle_connection(mut stream: TcpStream) {

    let mut encoded_vector = vec![];

    loop {
        let mut buffer = [0; 1024];

        thread::sleep(Duration::from_millis(10));

        let read_bytes = stream.read(&mut buffer).unwrap();

        encoded_vector.append(&mut buffer[..read_bytes].to_vec());


        if read_bytes < 1024 {
            break;
        }
    }
    let string_request = String::from_utf8(encoded_vector).unwrap();

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
        ("GET", ["articles", id, "comments"]) => comment::get_comments_for_article(id, &request),
        ("POST", ["articles", id, "comments"]) => comment::post_comment_route(id, &request),
        ("PUT", ["comments", id]) => comment::put_comment(id, &request),
        ("GET", ["create-article"]) => HttpResponse::from_file(StatusCode::Ok, "create-article.html"),
        ("GET", ["list-article"]) => HttpResponse::from_file(StatusCode::Ok, "list-article.html"),
        ("GET", ["see-article", uuid]) => legacy::see_article(uuid, &request),
        ("GET", ["edit-article", uuid]) => legacy::edit_article(uuid, &request),
        ("GET", ["articles"]) => article::get_articles_route(&request),
        ("GET", ["articles", uuid]) => article::get_article_route(uuid),
        ("POST", ["articles"]) => article::post_articles_route(&request),
        ("PUT", ["articles", uuid]) => article::put_article_route(uuid, &request),
        ("GET", ["public", file_name]) => HttpResponse::from_file(StatusCode::Ok, file_name),
        _ => HttpResponse::from_file(StatusCode::NotFound, "404.html")
    }
}
