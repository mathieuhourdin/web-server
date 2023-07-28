use std::collections::HashMap;
use std::fs;
use crate::entities::session::Session;
use crate::environment::get_api_url;
use std::time::{SystemTime, Duration};
use chrono::{DateTime, Local};

pub struct ServerUrl {
    pub protocol: String,
    pub host: String,
    pub port: Option<String>,
}

impl ServerUrl {
    pub fn from(url_string: String) -> ServerUrl {
        let splitted_first = url_string.split("://").collect::<Vec<&str>>();
        let protocol = splitted_first.get(0).expect("Should have a protocol value in url").to_string();
        let authority = splitted_first.get(1).expect("Should have an authority");
        let host = authority.split(":").collect::<Vec<_>>().get(0).expect("Should have a host").to_string();
        let port = authority.split(":").collect::<Vec<_>>().get(1).map(|value| value.to_string());
        ServerUrl { protocol, host, port }
    }

    pub fn to_string_url(&self) -> String {
        match &self.port {
            Some(value) => format!("{}://{}:{}", self.protocol, self.host, value),
            None => format!("{}://{}", self.protocol, self.host),
        }
    }
}

pub struct HttpRequest {
    pub method: String,
    pub uri: String,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub cookies: Cookie,
    pub session: Option<Session>
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

        let cookies = Cookie { data: HashMap::new() };

        HttpRequest { method, uri, headers, body, cookies, session: None }
    }

    pub fn parsed_uri(&self) -> Vec<&str> {
        self.uri[1..].split("/").collect::<Vec<&str>>()
    }
}

pub struct HttpResponse {
    pub status_code: StatusCode,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl HttpResponse {
    pub fn from(status_code: StatusCode, headers: HashMap<String, String>, body: String) -> HttpResponse {
        let mut headers = headers.clone();
        headers.insert("Access-Control-Allow-Origin".to_string(), ServerUrl::from(get_api_url()).host);
        headers.insert("Access-Control-Allow-Credentials".to_string(), "true".to_string());
        HttpResponse { status_code, headers, body }
    }

    pub fn from_file(status_code: StatusCode, file_path: &str) -> HttpResponse {
        let headers = HashMap::new();
        let html_body = fs::read_to_string(format!("public/{file_path}")).unwrap();
        let html_body = html_body.replace("process.env.API_URL", format!("'{}'", get_api_url()).as_str());
        HttpResponse::from(status_code, headers, html_body)
    }

    pub fn set_header(&mut self, header_title: &str, header_content: String) {
        self.headers.insert(String::from(header_title), header_content);
    }

    pub fn to_stream(&self) -> String {
        let status_line = self.status_code.to_status_line();
        let content_size = self.body.len();
        let mut headers_string = String::new();
        for (key, value) in &self.headers {
            headers_string += &format!("\r\n{key}: {value}")[..];
        }
        format!("{status_line}{headers_string}\r\nContent-Length: {content_size}\r\n\r\n{}", self.body)
    }
}

pub enum StatusCode {
    Ok,
    Created,
    BadRequest,
    Unauthorized,
    NotFound,
    InternalServerError
}

impl StatusCode {
    pub fn to_status_line(&self) -> &str {
        match self {
            StatusCode::Ok => "HTTP/1.1 200 OK",
            StatusCode::Created => "HTTP/1.1 201 CREATED",
            StatusCode::BadRequest => "HTTP/1.1 400 BAD REQUEST",
            StatusCode::Unauthorized => "HTTP/1.1 401 UNAUTHORIZED",
            StatusCode::NotFound => "HTTP/1.1 404 NOT FOUND",
            StatusCode::InternalServerError => "HTTP/1.1 500 INTERNAL SERVER ERROR",
        }
    }
}

pub struct Cookie {
    pub data: HashMap<String, String>
}

impl Cookie {
    pub fn from(string_cookie: Option<&String>) -> Cookie {
        match string_cookie {
            None => Cookie { data: HashMap::new() },
            Some(value) => {
                let mut data_hashmap = HashMap::new();
                let splitted_values = value.split("; ").collect::<Vec<&str>>();
                for value in splitted_values {
                    let splitted_cookie_value = value.split("=").collect::<Vec<&str>>();
                    if let Some(key) = splitted_cookie_value.get(0) {
                        if let Some(value) = splitted_cookie_value.get(1) {
                            data_hashmap.insert(key.to_string(), value.to_string());
                        }
                    }
                }
                Cookie { data: data_hashmap }
            }
        }
    }
}

pub struct CookieValue {
    pub key: String,
    pub value: String,
    pub expires: SystemTime,
    pub domain: String,
    pub path: String,
    pub same_site: String,
    pub secure: bool,
}

impl CookieValue {
    pub fn new(key: String, value: String, expires: SystemTime) -> CookieValue {
        let domain = ServerUrl::from(get_api_url()).host;
        let path = "/".to_string();
        let same_site = "Lax".to_string();
        let secure = true;
        CookieValue { key, value, expires, domain, path, same_site, secure }
    }

    pub fn format(&self) -> String {

        let datetime: DateTime<Local> = self.expires.into();
        let expires = datetime.format("%a, %d-%b-%Y %H:%M:%S GMT");
        format!("{}={}; Expires={expires}; Domain={}; Path={}; SameSite={}; Secure", self.key, self.value, self.domain, self.path, self.same_site)
    }
}
