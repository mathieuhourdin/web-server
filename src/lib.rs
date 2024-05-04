use std::net::{TcpStream};
use std::io::{prelude::*};
use std::thread;
use http::{HttpRequest, HttpResponse, StatusCode};
use std::time::{Duration};
use chrono::{Utc};
use middlewares::cors_middleware;

pub mod threadpool;
pub mod entities;
pub mod http;
pub mod sessions_service;
pub mod environment;
pub mod db;
pub mod schema;
pub mod legacy;
pub mod router;
pub mod middlewares;
pub mod link_preview;
pub mod file_converter;

pub async fn handle_connection(mut stream: TcpStream) {

    let current_time = Utc::now().naive_utc();
    println!("[handle_connection lib.rs] Current time : {:#?}", current_time);

    if let Err(_err) = stream.set_read_timeout(Some(Duration::from_secs(30))) {
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
                let response = HttpResponse::new(StatusCode::BadRequest, "handle_connection : Unable to decode request".to_string());
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

    let mut http_request = match HttpRequest::from_bytes(encoded_vector) {
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

