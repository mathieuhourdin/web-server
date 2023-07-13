use std::net::{TcpStream};
use std::io::{prelude::*};
use std::fs;
use std::thread;
use std::time::Duration;
use std::collections::HashMap;
use serde_json;

pub mod threadpool;
pub mod httpclient;
pub mod database;
pub mod entities;

pub struct HttpRequest {
    pub method: String,
    pub uri: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl HttpRequest {
    pub fn from(request_string: &String) -> HttpRequest {

        let request_array = request_string.split("\r\n\r\n").collect::<Vec<&str>>();

        if request_array.len() < 2 {
            panic!("Should be a valid http request");
        }

        let first_part = &request_array[0].split("\r\n").collect::<Vec<&str>>();

        let first_row = first_part[0].split(" ").collect::<Vec<&str>>();
        let method = String::from(first_row[0]);
        let uri = String::from(first_row[1]);
        println!("Method : {method}");

        let mut headers = HashMap::new();
        for row in &first_part[1..] {
            let parsed_header = row.split(": ").collect::<Vec<&str>>();
            headers.insert(String::from(parsed_header[0]), String::from(parsed_header[1]));
            println!("Header : {row}");
        }
        let body = String::from(request_array[1]);

        for line in request_array {
            println!("Line : {line}");
        }

        HttpRequest { method, uri, headers, body }
    }
}

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

    let (status_line, contents) = response_from_request(http_request).await;

    let content_size = contents.len();

    let response = format!("{status_line}\r\nContent-Length: {content_size}\r\n\r\n{contents}");

    stream.write_all(response.as_bytes()).unwrap();
}

async fn response_from_request(request: HttpRequest) -> (String, String) {
    match (&request.method[..], &request.uri[..]) {
        ("GET", "/") => response_from_file(200, "hello.html").await,
        ("GET", "/mathilde") => response_from_file(200, "mathilde.html").await,
        ("POST", "/mathilde") => post_mathilde_route(),
        ("POST", "/articles") => post_articles_route(&request.body).await,
        ("GET",_) if request.uri[..].starts_with("/articles/") => get_article_route(&request.uri[..]).await,
        ("GET", "/sleep") => sleep_route(),
        _ => response_from_file(404, "404.html").await
    }
}

async fn response_from_file(status_code: u32, file_name: &str) -> (String, String) {
    println!("Response from file route");
    (format_status_line_from_code(status_code), fs::read_to_string(file_name).unwrap())
}

async fn get_article_route(article_uuid: &str) -> (String, String) {
    println!("Article uuid : {article_uuid}");
    let uuid = article_uuid.split("/").collect::<Vec<&str>>()[2];
    (format_status_line_from_code(200), serde_json::to_string(&database::get_article(uuid).await.unwrap()).unwrap())
}

async fn post_articles_route(body: &String) -> (String, String) {

    match serde_json::from_str(&body[..]) {
        Ok(mut article) => (format_status_line_from_code(200), database::create_article(&mut article).await.unwrap()),
        Err(err) => return (format_status_line_from_code(400), format!("Error with the payload : {:#?}", err)),
    }
}

fn post_mathilde_route() -> (String, String) {
    
    (format_status_line_from_code(201), String::from("Cool but I already have one"))
}

fn sleep_route() -> (String, String) {
    thread::sleep(Duration::from_secs(10));
    (String::from(format_status_line_from_code(200)), String::from("hello.html"))
}

// should use a struct constructor instead
fn format_status_line_from_code(status_code: u32) -> String {
    String::from(match status_code {
        200 => "HTTP/1.1 200 OK",
        201 => "HTTP/1.1 201 CREATED",
        404 => "HTTP/1.1 404 NOT FOUND",
        400 => "HTTP/1.1 400 BAD REQUEST",
        _ => panic!("Supposed to have a known status code")
    })
}

