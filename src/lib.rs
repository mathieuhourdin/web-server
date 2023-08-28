use std::net::{TcpStream};
use std::io::{prelude::*};
use std::thread;
use std::time::Duration;
use serde_json;
use http::{HttpRequest, HttpResponse, StatusCode, Cookie};
use entities::{user::{User, NewUser}, article::Article, error::PpdcError};
use regex::Regex;

pub mod threadpool;
pub mod database;
pub mod entities;
pub mod http;
pub mod sessions_service;
pub mod environment;
pub mod db;
pub mod schema;

pub async fn handle_connection(mut stream: TcpStream) {

    let mut string_request = String::new();

    loop {
        let mut buffer = [0; 1024];

        thread::sleep(Duration::from_millis(10));

        let read_bytes = stream.read(&mut buffer).unwrap();

        string_request += &String::from(String::from_utf8_lossy(&buffer[..read_bytes]))[..];

        if read_bytes < 1024 {
            break;
        }
    }

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
    let mut response = decode_cookie_for_request_middleware(request).await;
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
        ("POST", ["mathilde"]) => post_mathilde_route(&request).await,
        ("GET", ["users", uuid]) => get_user_by_uuid(uuid).await,
        ("POST", ["users"]) => post_user(&request).await,
        ("GET", ["login"]) => HttpResponse::from_file(StatusCode::Ok, "login.html"),
        ("POST", ["sessions"]) => sessions_service::post_session_route(request).await,
        ("GET", ["sessions"]) => sessions_service::get_session_route(request).await,
        ("POST", ["articles"]) => post_articles_route(&request).await,
        ("GET", ["create-article"]) => HttpResponse::from_file(StatusCode::Ok, "create-article.html"),
        ("GET", ["list-article"]) => HttpResponse::from_file(StatusCode::Ok, "list-article.html"),
        ("GET", ["see-article", uuid]) => see_article(uuid, &request),
        ("GET", ["edit-article", uuid]) => edit_article(uuid, &request),
        ("GET", ["articles"]) => get_articles(&request).await,
        ("GET", ["articles", uuid]) => get_article_route(uuid).await,
        ("PUT", ["articles", uuid]) => put_article_route(uuid, &request).await,
        ("GET", ["sleep"]) => sleep_route(),
        ("GET", ["public", file_name]) => HttpResponse::from_file(StatusCode::Ok, file_name),
        ("OPTIONS", [..]) => option_response(&request),
        _ => HttpResponse::from_file(StatusCode::NotFound, "404.html")
    }
}

fn option_response(_request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let mut response = HttpResponse::new(StatusCode::Ok, "Ok".to_string());
    response.headers.insert("Access-Control-Allow-Headers".to_string(), "authorization, content-type".to_string());
    Ok(response)
}

async fn post_user(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let mut user_message = serde_json::from_str::<NewUser>(&request.body[..])?;
    user_message.hash_password().unwrap();
    let created_user = User::create(user_message)?;
    Ok(HttpResponse::new(
        StatusCode::Created,
        serde_json::to_string(&created_user).unwrap()
    ))
}

async fn get_user_by_uuid(user_uuid: &str) -> Result<HttpResponse, PpdcError> {
    Ok(HttpResponse::new(
        StatusCode::Ok,
        serde_json::to_string(&User::find(&HttpRequest::parse_uuid(user_uuid)?)?)?
        ))
}

fn see_article(uuid: &str, _request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let mut response = HttpResponse::from_file(StatusCode::Ok, "article_id.html")?;
    response.body = response.body.replace("INJECTED_ARTICLE_ID", format!("'{}'", uuid).as_str());
    Ok(response)
}

fn edit_article(uuid: &str, _request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let mut response = HttpResponse::from_file(StatusCode::Ok, "edit-article-id.html")?;
    response.body = response.body.replace("INJECTED_ARTICLE_ID", format!("'{}'", uuid).as_str());
    Ok(response)
}

async fn put_article_route(uuid: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {

    println!("Put_article_route with uuid : {:#?}", uuid);

    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::new(StatusCode::Unauthorized, "user should be authentified".to_string()));
    }
    let mut article = serde_json::from_str::<Article>(&request.body[..])?;
    article.author_id = Some(request.session.as_ref().unwrap().user_id.as_ref().unwrap().to_string().clone());
    Ok(HttpResponse::new(
        StatusCode::Ok,
        database::update_article(&mut article)
        .await
        .map(|()| "Ok".to_string()).unwrap()))
}

async fn get_articles(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let limit: u32 = request.query.get("limit").unwrap_or(&"20".to_string()).parse().unwrap();
    let offset: u32 = request.query.get("offset").unwrap_or(&"0".to_string()).parse().unwrap();
    let articles = &database::get_articles(offset, limit).await?;
    Ok(HttpResponse::new(
       StatusCode::Ok,
       serde_json::to_string(articles).unwrap()))
}

async fn get_article_route(article_uuid: &str) -> Result<HttpResponse, PpdcError> {
    println!("Article uuid : {article_uuid}");
    Ok(HttpResponse::new(
        StatusCode::Ok,
        serde_json::to_string(&database::get_article(article_uuid)
            .await
            .unwrap())
        .unwrap()))
}

async fn post_articles_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {

    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::new(StatusCode::Unauthorized, "user should be authentified".to_string()));
    }
    let mut article = serde_json::from_str::<Article>(&request.body[..])?;
    article.author_id = Some(request.session.as_ref().unwrap().user_id.as_ref().unwrap().to_string().clone());
    Ok(HttpResponse::new(
        StatusCode::Ok,
        database::create_article(&mut article)
        .await
        .unwrap()))
}

async fn post_mathilde_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {

    match &request.session.as_ref().unwrap().user_id {
        Some(user_id) => {
            let user_first_name = User::find(user_id)?.first_name;
            Ok(HttpResponse::new(
                StatusCode::Created,
                format!("Cool {user_first_name} but I already have one")))
        },
        None => Ok(HttpResponse::new(StatusCode::Unauthorized, String::from("Not authenticated request"))),
    }
    
}

fn sleep_route() -> Result<HttpResponse, PpdcError> {
    thread::sleep(Duration::from_secs(10));
    Ok(HttpResponse::new(StatusCode::Ok, String::from("hello.html")))
}
