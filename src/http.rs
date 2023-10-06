use std::collections::HashMap;
use std::fs;
use crate::entities::session::Session;
use crate::environment::get_api_url;
use chrono::{NaiveDateTime};
use crate::entities::error::{PpdcError, ErrorType};
use uuid::Uuid;
use serde_json;
use serde::Serialize;

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
    pub path: String,
    pub headers: HashMap<String, String>,
    pub query: HashMap<String, String>,
    pub body: String,
    pub cookies: Cookie,
    pub session: Option<Session>
}

fn decode_query_string(query_string: &str) -> HashMap<String, String> {
    let mut query_hash_map = HashMap::new();
    let splitted = query_string.split("&").collect::<Vec<&str>>();
    for query in splitted {
        let vec_value = query.split("=").collect::<Vec<&str>>();
        if let Some(key) = vec_value.get(0) {
            if let Some(value) = vec_value.get(1) {
                query_hash_map.insert(key.to_string(), value.to_string());
            }
        }
    }
    query_hash_map
}

impl HttpRequest {
    pub fn from(request_string: &String) -> Result<HttpRequest, PpdcError> {

        let request_array = request_string.split("\r\n\r\n").collect::<Vec<&str>>();

        if request_array.len() < 2 {
            return Err(PpdcError::new(500, ErrorType::InternalError, format!("Not enouth lines in request : {:#?}", request_string)));
        }

        let first_part = &request_array[0].split("\r\n").collect::<Vec<&str>>();

        let first_row = first_part[0].split(" ").collect::<Vec<&str>>();
        let method = String::from(first_row[0]);

        let uri = first_row[1];

        let path = uri.split("?").collect::<Vec<&str>>().get(0).expect("Should have a uri").to_string();
        let query = if let Some(query_string) = uri.split("?").collect::<Vec<&str>>().get(1) {
            decode_query_string(query_string)
        } else {
            HashMap::new()
        };

        println!("Method : {method}");

        let mut headers = HashMap::new();
        for row in &first_part[1..] {
            let parsed_header = row.split(": ").collect::<Vec<&str>>();
            headers.insert(
                parsed_header[0]
                    .to_string()
                    .to_lowercase(), // convert header keys to lowercase. Compatible with http2
                parsed_header[1]
                    .to_string()); 
            println!("Header : {row}");
        }
        let body = String::from(request_array[1]);

        for line in request_array {
            println!("Line : {line}");
        }

        let cookies = Cookie { data: HashMap::new() };

        Ok(HttpRequest { method, path, query, headers, body, cookies, session: None })
    }

    pub fn parsed_path(&self) -> Vec<&str> {
        self.path[1..].split("/").collect::<Vec<&str>>()
    }

    pub fn parse_uuid(uuid: &str) -> Result<Uuid, PpdcError> {
        Uuid::parse_str(uuid).map_err(|err| PpdcError::new(400, ErrorType::ApiError, format!("Incorrect uuid for ressource : {:#?}", err)))
    }

    pub fn get_pagination(&self) -> (i64, i64) {
        let offset: i64 = self.query.get("offset").unwrap_or(&"0".to_string()).parse().unwrap();
        let limit: i64 = self.query.get("limit").unwrap_or(&"20".to_string()).parse().unwrap();
        (offset, limit)
    }
}

pub struct HttpResponse {
    pub status_code: StatusCode,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl HttpResponse {
    pub fn new(status_code: StatusCode, body: String) -> HttpResponse {
        let headers = HashMap::new();
        HttpResponse { status_code, headers, body }
    }

    pub fn ok() -> HttpResponse {
        HttpResponse::new(StatusCode::Ok, String::new())
    }

    pub fn created() -> HttpResponse {
        HttpResponse::new(StatusCode::Created, String::new())
    }

    pub fn unauthorized() -> HttpResponse {
        HttpResponse::new(StatusCode::Unauthorized, "User should be authenticated".to_string())
    }

    pub fn body(mut self, body: String) -> HttpResponse {
        self.body = body;
        self
    }

    pub fn json<T>(mut self, body: &T) -> Result<HttpResponse, PpdcError>
        where
            T: ?Sized + Serialize,
    {
        self.body = serde_json::to_string(body)?;
        Ok(self)
    }

    pub fn header(mut self, key: &str, value: String) -> HttpResponse {
        self.headers.insert(key.to_string(), value);
        self
    }

    pub fn from_file(status_code: StatusCode, file_path: &str) -> Result<HttpResponse, PpdcError> {
        let html_body = fs::read_to_string(format!("public/{file_path}")).unwrap();

        let header_code = fs::read_to_string("public/Header.html").unwrap();
        let html_body = html_body.replace("<Header />", &header_code);

        let user_management_code = fs::read_to_string("public/user-datas.html").unwrap();
        let html_body = html_body + &user_management_code;

        let html_body = html_body.replace("process.env.API_URL", format!("'{}'", get_api_url()).as_str());
        Ok(HttpResponse::new(status_code, html_body))
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
    pub expires: NaiveDateTime,
    pub domain: String,
    pub path: String,
    pub same_site: String,
    pub secure: bool,
}

impl CookieValue {
    pub fn new(key: String, value: String, expires: NaiveDateTime) -> CookieValue {
        let domain = ServerUrl::from("http://localhost:5173".to_string()).host;
        let path = "/".to_string();
        let same_site = "Lax".to_string();
        let secure = true;
        CookieValue { key, value, expires, domain, path, same_site, secure }
    }

    pub fn format(&self) -> String {

        let datetime: NaiveDateTime = self.expires.into();
        let expires = datetime.format("%a, %d-%b-%Y %H:%M:%S GMT");
        format!("{}={}; Expires={expires}; Domain={}; Path={}; SameSite={}", self.key, self.value, self.domain, self.path, self.same_site)
    }
}
