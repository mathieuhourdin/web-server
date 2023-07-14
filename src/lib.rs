use std::net::{TcpStream};
use std::io::{prelude::*};
use std::fs;
use std::thread;
use std::time::Duration;
use serde_json;
use http::{HttpRequest, HttpResponse, StatusCode};
use std::collections::HashMap;

pub mod threadpool;
pub mod database;
pub mod entities;
pub mod http;

pub async fn handle_connection(mut stream: TcpStream) {

    let mut request = String::new();

    loop {
        let mut buffer = [0; 1024];

        let read_bytes = stream.read(&mut buffer).unwrap();

        request += &String::from(String::from_utf8_lossy(&buffer[..read_bytes]))[..];

        if read_bytes < 1024 {
            break;
        }
    }

    println!("Request is : {request}");

    let http_request = HttpRequest::from(&request);

    let http_response: HttpResponse = response_from_request(http_request).await;

    let response = http_response.to_stream(); 
    stream.write_all(response.as_bytes()).unwrap();
}

async fn response_from_request(request: HttpRequest) -> HttpResponse {
    match (&request.method[..], &request.uri[..]) {
        ("GET", "/") => response_from_file(StatusCode::Ok, "hello.html").await,
        ("GET", "/mathilde") => response_from_file(StatusCode::Ok, "mathilde.html").await,
        ("POST", "/mathilde") => post_mathilde_route(),
        ("POST", "/articles") => post_articles_route(&request.body).await,
        ("GET",_) if request.uri[..].starts_with("/articles/") => get_article_route(&request.uri[..]).await,
        ("GET", "/sleep") => sleep_route(),
        _ => response_from_file(StatusCode::NotFound, "404.html").await
    }
}

async fn response_from_file(status_code: StatusCode, file_name: &str) -> HttpResponse {
    println!("Response from file route");
    HttpResponse::from(status_code, HashMap::new(), fs::read_to_string(file_name).unwrap())
}

async fn get_article_route(article_uuid: &str) -> HttpResponse {
    println!("Article uuid : {article_uuid}");
    let uuid = article_uuid.split("/").collect::<Vec<&str>>()[2];
    HttpResponse::from(
        StatusCode::Ok,
        HashMap::new(),
        serde_json::to_string(&database::get_article(uuid)
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

fn post_mathilde_route() -> HttpResponse {
    
    HttpResponse::from(StatusCode::Created, HashMap::new(), String::from("Cool but I already have one"))
}

fn sleep_route() -> HttpResponse {
    thread::sleep(Duration::from_secs(10));
    HttpResponse::from(StatusCode::Ok, HashMap::new(),String::from("hello.html"))
}
