
use std::net::{TcpStream};
use std::io::{prelude::*};
use std::thread;
use crate::http::{HttpRequest, HttpResponse, StatusCode};
use std::time::{Duration};
use chrono::{Utc};
use crate::middlewares::cors_middleware;

pub async fn handle_connexion(mut stream: TcpStream) {

    let current_time = Utc::now().naive_utc();
    println!("[handle_connection lib.rs] Current time : {:#?}", current_time);

    if let Err(_err) = stream.set_read_timeout(Some(Duration::from_secs(30))) {
        println!("Error setting read timeout");
        return;
    }

    let mut encoded_vector = vec![];

    loop {
        let mut buffer = [0; 1024];

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
        } else {
            thread::sleep(Duration::from_millis(1));
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

    http_request.set_received_at(current_time);

    let request_created_at = http_request.created_at;
    let request_received_at = http_request.received_at.unwrap();

    let http_response: &mut HttpResponse = &mut cors_middleware(&mut http_request).await;

    let end_time = Utc::now().naive_utc();

    http_response.headers.insert("time_from_reception".to_string(), (end_time - request_received_at).num_milliseconds().to_string() + "ms");
    http_response.headers.insert("time_from_request_creation".to_string(), (end_time - request_created_at).num_milliseconds().to_string() + "ms");



    let response = http_response.to_stream(); 
    stream.write_all(response.as_bytes()).unwrap();
}
