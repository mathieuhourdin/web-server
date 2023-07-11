use std::io::prelude::*;
use std::net::TcpStream;

pub struct Client {
    socket_address: String
}

impl Client {
    pub fn new(socket_address: String) -> Client {
        Client { socket_address }
    }

    pub fn request(&self, method: &str, url: &str, payload: String) -> String {

        let mut stream = TcpStream::connect(&self.socket_address[..]).unwrap();

        let message = create_request_string(method, url, &self.socket_address[..], payload);

        stream.write_all(message.as_bytes()).unwrap();

        let mut response_buffer = Vec::new();
        stream.read_to_end(&mut response_buffer).unwrap();

        let string_response = String::from_utf8_lossy(&response_buffer);

        String::from(string_response)

    }

    pub fn get(&self, url: &str) -> String {
        self.request("GET", url, String::from(""))
    }

    pub fn post(&self, url: &str, payload: String) -> String {
        self.request("POST", url, payload)
    }
}

fn create_request_string(method: &str, url_path: &str, socket_address: &str, payload: String) -> String {
    format!("{method} {url_path} HTTP/1.1\r\nHost: {socket_address}\r\nUser-Agent: client/1.0.0\r\n\r\n{payload}")
}
        
