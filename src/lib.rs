use std::net::{TcpStream};
use std::io::{prelude::*};
use std::thread;
use std::time::Duration;
use serde_json;
use http::{ServerUrl, HttpRequest, HttpResponse, StatusCode, Cookie};
use std::collections::HashMap;
use entities::user::User;

pub mod threadpool;
pub mod database;
pub mod entities;
pub mod http;
pub mod sessions_service;
pub mod environment;

pub async fn handle_connection(mut stream: TcpStream) {

    let mut string_request = String::new();

    loop {
        let mut buffer = [0; 1024];

        let read_bytes = stream.read(&mut buffer).unwrap();

        string_request += &String::from(String::from_utf8_lossy(&buffer[..read_bytes]))[..];

        if read_bytes < 1024 {
            break;
        }
    }

    println!("Request is : {string_request}");

    let mut http_request = HttpRequest::from(&string_request);


    let http_response: HttpResponse = decode_cookie_for_request_middleware(&mut http_request).await;

    let response = http_response.to_stream(); 
    stream.write_all(response.as_bytes()).unwrap();
}

async fn decode_cookie_for_request_middleware(request: &mut HttpRequest) -> HttpResponse {
    let request_cookies = request.headers.get("Cookie");
    println!("Request cookies : {:#?}", request_cookies);
    let formatted_cookie = Cookie::from(request_cookies);
    request.cookies = formatted_cookie;
    add_session_to_request_middleware(request).await
}

async fn add_session_to_request_middleware(request: &mut HttpRequest) -> HttpResponse {
    let session_cookie = match sessions_service::create_or_attach_session(request).await {
        Ok(value) => value,
        Err(err) => return HttpResponse::from(
            StatusCode::InternalServerError,
            HashMap::new(),
            format!("Error when creating session: {:#?}", err.message)
            )
    };
    let mut response = route_request(request).await;
    println!("Session cookie : {session_cookie}");
    response.set_header("Set-Cookie", session_cookie);
    response
}

async fn route_request(request: &mut HttpRequest) -> HttpResponse {
    let session_id = &request.session.as_ref().unwrap().id;
    println!("Request with session id : {session_id}");
    match (&request.method[..], &request.parsed_path()[..]) {
        ("GET", [""]) => HttpResponse::from_file(StatusCode::Ok, "hello.html"),
        ("GET", ["mathilde"]) => HttpResponse::from_file(StatusCode::Ok, "mathilde.html"),
        ("POST", ["mathilde"]) => post_mathilde_route(&request).await,
        ("GET", ["users", uuid]) => get_user_by_uuid(uuid).await,
        ("POST", ["users"]) => post_user(&request).await,
        ("POST", ["sessions"]) => sessions_service::post_session_route(request).await,
        ("POST", ["articles"]) => post_articles_route(&request.body).await,
        ("GET", ["articles"]) => get_articles(&request).await,
        ("GET", ["articles", uuid]) => get_article_route(uuid).await,
        ("GET", ["sleep"]) => sleep_route(),
        ("GET", ["public", file_name]) => HttpResponse::from_file(StatusCode::Ok, file_name),
        _ => HttpResponse::from_file(StatusCode::NotFound, "404.html")
    }
}

async fn post_user(request: &HttpRequest) -> HttpResponse {
    match serde_json::from_str::<User>(&request.body[..]) {
        Ok(mut user) => {
            user.hash_password().unwrap();
            HttpResponse::from(
                StatusCode::Created,
                HashMap::new(),
                database::create_user(&mut user).await.unwrap()
            )
        },
        Err(err) => HttpResponse::from(
            StatusCode::BadRequest,
            HashMap::new(),
            format!("Error with the payload: {:#?}", err))
    }
}

async fn get_user_by_uuid(user_uuid: &str) -> HttpResponse {
    HttpResponse::from(
        StatusCode::Ok,
        HashMap::new(),
        serde_json::to_string(&database::get_user_by_uuid(user_uuid).await.unwrap()).unwrap()
        )
}

async fn get_articles(request: &HttpRequest) -> HttpResponse {
    if let Some(limit) = request.query.get("limit") {
        println!("Limit : {limit}");
    }
    HttpResponse::from(StatusCode::Ok, HashMap::new(), "Ok".to_string())
}

async fn get_article_route(article_uuid: &str) -> HttpResponse {
    println!("Article uuid : {article_uuid}");
    HttpResponse::from(
        StatusCode::Ok,
        HashMap::new(),
        serde_json::to_string(&database::get_article(article_uuid)
            .await
            .unwrap())
        .unwrap())
}

async fn post_articles_route(body: &String) -> HttpResponse {

    match serde_json::from_str(&body[..]) {
        Ok(mut article) => HttpResponse::from(
            StatusCode::Ok,
            HashMap::new(),
            database::create_article(&mut article)
            .await
            .unwrap()),
        Err(err) => HttpResponse::from(
            StatusCode::BadRequest,
            HashMap::new(),
            format!("Error with the payload : {:#?}", err)),
    }
}

async fn post_mathilde_route(request: &HttpRequest) -> HttpResponse {

    match &request.session.as_ref().unwrap().user_id {
        Some(user_id) => {
            let user_first_name = match database::get_user_by_uuid(user_id).await {
                Ok(user) => user.first_name,
               Err(err) => return HttpResponse::from(
                   StatusCode::InternalServerError,
                   HashMap::new(),
                   format!("Cannot get user with uuid : {user_id}; error : {:#?}", err)
                   )
            };
            HttpResponse::from(
                StatusCode::Created,
                HashMap::new(),
                format!("Cool {user_first_name} but I already have one"))
        },
        None => HttpResponse::from(StatusCode::Unauthorized, HashMap::new(), String::from("Not authenticated request")),
    }
    
}

fn sleep_route() -> HttpResponse {
    thread::sleep(Duration::from_secs(10));
    HttpResponse::from(StatusCode::Ok, HashMap::new(),String::from("hello.html"))
}
