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

#[derive(Debug)]
pub enum ContentType {
    ApplicationJson,
    FormData
}

#[derive(Debug)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub query: HashMap<String, String>,
    pub body: String,
    pub cookies: Cookie,
    pub session: Option<Session>,
    pub content_type: Option<ContentType>,
    pub delimiter: Option<String>
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
    pub fn new(method: &str, path: &str, body: &str) -> HttpRequest {
        HttpRequest {
            method: method.to_string(),
            path: path.to_string(),
            headers: HashMap::new(),
            query: HashMap::new(),
            body: body.to_string(),
            cookies: Cookie { data: HashMap::new() },
            session: None,
            content_type: None,
            delimiter: None
        }
    }

    pub fn parse_request_first_line(&mut self, request_first_line: &str) -> Result<(), PpdcError> {
        let mut method_line = request_first_line.split(" ");
        let method = method_line.next().ok_or_else(|| {
            return PpdcError::new(500, ErrorType::InternalError, format!("First line is empty : {:#?}", request_first_line));
        })?.to_string();
        self.method = method;
        let mut uri = method_line.next().ok_or_else(|| {
            return PpdcError::new(500, ErrorType::InternalError, format!("Request has no uri : {:#?}", request_first_line));
            })?
            .to_string();
        let mut uri = uri.split("?");
        println!("Method line : {:?}", method_line);
        let path = uri.next().ok_or_else(|| {
            return PpdcError::new(500, ErrorType::InternalError, format!("Request has no path : {:#?}", request_first_line));
        })?.to_string();
        self.path = path;
        if let Some(query) = uri.next() {
            self.query = decode_query_string(query);
        }

        Ok(())
    }

    pub fn from(request_string: &String) -> Result<HttpRequest, PpdcError> {

        let mut request_lines_iterator = request_string.split("\r\n");
        let mut request = HttpRequest::new("", "", "");
        

        let method_line = request_lines_iterator.next().ok_or_else(|| {
            return PpdcError::new(500, ErrorType::InternalError, format!("Request is empty : {:#?}", request_string));
        })?;

        request.parse_request_first_line(method_line)?;


        for row in &mut request_lines_iterator {
            if row == "" {
                break;
            }
            let row = row.split(": ").collect::<Vec<&str>>();
            request.headers.insert(row[0].to_string().to_lowercase(), row[1].to_string());
        }

        if let Some(content_type) = request.headers.get("content-type") {
            if *&content_type.len() > 19 && &content_type[..19] == "multipart/form-data" {
                request.content_type = Some(ContentType::FormData);
                request.delimiter = Some(content_type.split("boundary=").collect::<Vec<&str>>()[1].to_string());
            } else {
                request.content_type = Some(ContentType::ApplicationJson);
            }
        }

        for row in request_lines_iterator {
            request.body = request.body + row + "\r\n"
        }

        println!("Request : {:?}", request);

        Ok(request)
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

#[derive(Debug, PartialEq)]
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
        let html_body = html_body + user_management_code.as_str();

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

#[derive(Debug, PartialEq)]
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

#[derive(Debug)]
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
