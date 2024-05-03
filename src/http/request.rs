use std::collections::HashMap;
use crate::entities::error::{PpdcError, ErrorType};
use uuid::Uuid;
use super::*;
use crate::entities::session::Session;

#[derive(Debug, PartialEq)]
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
        let uri = method_line.next().ok_or_else(|| {
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

//    pub fn from_bytes(request_data: &[u8]) -> Result<HttpRequest, PpdcError> {
//        let line_break_delimiter = b"\r\n";
//        let line_break_index = request_data.windows(2).position(|w| w == line_break_delimiter).unwrap();
//        let first_line = request_data[..line_break_index];
//    }

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
            if *&content_type.len() >= 19 && &content_type[..19] == "multipart/form-data" {
                request.content_type = Some(ContentType::FormData);
                request.delimiter = Some(content_type.split("boundary=").collect::<Vec<&str>>()[1].to_string());
            } else {
                request.content_type = Some(ContentType::ApplicationJson);
            }
        }

        println!("Continue script");

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

