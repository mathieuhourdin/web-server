use super::*;
use crate::entities::error::{ErrorType, PpdcError};
use crate::entities::session::Session;
use chrono::{NaiveDateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, PartialEq)]
pub enum ContentType {
    ApplicationJson,
    FormData,
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
    pub delimiter: Option<String>,
    pub files: Vec<File>,
    pub created_at: NaiveDateTime,
    pub received_at: Option<NaiveDateTime>,
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
            cookies: Cookie {
                data: HashMap::new(),
            },
            session: None,
            content_type: None,
            delimiter: None,
            files: Vec::new(),
            created_at: Utc::now().naive_utc(),
            received_at: None,
        }
    }

    pub fn parse_request_first_line(&mut self, request_first_line: &str) -> Result<(), PpdcError> {
        let mut method_line = request_first_line.split(" ");
        let method = method_line
            .next()
            .ok_or_else(|| {
                return PpdcError::new(
                    500,
                    ErrorType::InternalError,
                    format!("First line is empty : {:#?}", request_first_line),
                );
            })?
            .to_string();
        self.method = method;
        let uri = method_line
            .next()
            .ok_or_else(|| {
                return PpdcError::new(
                    500,
                    ErrorType::InternalError,
                    format!("Request has no uri : {:#?}", request_first_line),
                );
            })?
            .to_string();
        let mut uri = uri.split("?");
        println!("Method line : {:?}", method_line);
        let path = uri
            .next()
            .ok_or_else(|| {
                return PpdcError::new(
                    500,
                    ErrorType::InternalError,
                    format!("Request has no path : {:#?}", request_first_line),
                );
            })?
            .to_string();
        self.path = path;
        if let Some(query) = uri.next() {
            self.query = decode_query_string(query);
        }

        Ok(())
    }

    pub fn parse_headers(&mut self, headers_lines: String) -> Result<(), PpdcError> {
        let mut headers_lines_iterator = headers_lines.split("\r\n");
        dbg!(&headers_lines_iterator);
        for row in &mut headers_lines_iterator {
            let row = row.split(": ").collect::<Vec<&str>>();
            if row.len() > 1 {
                self.headers
                    .insert(row[0].to_string().to_lowercase(), row[1].to_string());
            }
        }

        if let Some(content_type) = self.headers.get("content-type") {
            if *&content_type.len() >= 19 && &content_type[..19] == "multipart/form-data" {
                self.content_type = Some(ContentType::FormData);
                self.delimiter =
                    Some(content_type.split("boundary=").collect::<Vec<&str>>()[1].to_string());
            } else {
                self.content_type = Some(ContentType::ApplicationJson);
            }
        }
        Ok(())
    }

    pub fn from_bytes(request_data: Vec<u8>) -> Result<HttpRequest, PpdcError> {
        let mut request = HttpRequest::new("", "", "");
        let line_break_delimiter = b"\r\n";
        let first_line_break_index = request_data
            .windows(2)
            .position(|w| w == line_break_delimiter);

        if first_line_break_index.is_none() {
            return Err(PpdcError::new(
                404,
                ErrorType::ApiError,
                "Not enougth lines in request".to_string(),
            ));
        }

        let first_line_break_index = first_line_break_index.unwrap();

        let first_line = &request_data[..first_line_break_index];
        let first_line = String::from_utf8(first_line.to_vec()).unwrap();
        request.parse_request_first_line(first_line.as_str())?;

        let headers_end_delimiter = b"\r\n\r\n";
        let header_end_index = request_data
            .windows(4)
            .position(|w| w == headers_end_delimiter)
            .unwrap();
        let header_lines: &[u8];
        if first_line_break_index == header_end_index {
            header_lines = &[];
        } else {
            header_lines = &request_data[first_line_break_index + 2..header_end_index];
        }
        let header_lines = String::from_utf8(header_lines.to_vec()).unwrap();
        dbg!(&header_lines);
        request.parse_headers(header_lines)?;

        dbg!(header_end_index);
        dbg!(request_data.len());
        let body_bytes = request_data[header_end_index + 4..].to_vec();

        // TODO need to implement FormData case
        if request.content_type == Some(ContentType::ApplicationJson) {
            let body_string_result = String::from_utf8(body_bytes.to_vec());
            request.body = match body_string_result {
                Err(_) => {
                    return Err(PpdcError::new(
                        404,
                        ErrorType::ApiError,
                        "handle_connection from_utf8 error : Unable to decode request".to_string(),
                    ));
                }
                Ok(body) => body,
            }
        } else if request.method.as_str() != "OPTIONS"
            && request.content_type == Some(ContentType::FormData)
        {
            let multipart_delimiter_string = request
                .delimiter
                .as_ref()
                .expect("Delimiter should be defined");

            dbg!(&multipart_delimiter_string);

            let multipart_delimiter = &multipart_delimiter_string.as_bytes();

            dbg!(&multipart_delimiter);
            dbg!(&body_bytes);
            //dbg!(&request_data);

            let multipart_end_delimiter = multipart_delimiter_string.to_owned() + "--";
            let multipart_end_delimiter = multipart_end_delimiter.as_bytes();
            let multipart_length = multipart_delimiter.len();

            let multipart_content_start_index = body_bytes
                .windows(multipart_length)
                .position(|w| w == *multipart_delimiter)
                .expect("Should have at least one delimiter");
            let multipart_content_end_index = body_bytes
                .windows(multipart_length + 2)
                .position(|w| w == multipart_end_delimiter)
                .expect("Should have a end delimiter");
            let mut multipart = &body_bytes
                [multipart_content_start_index + multipart_length..multipart_content_end_index];

            loop {
                let mut new_file = File::new();
                if let Some(next_form_data_start_index) = multipart
                    .windows(multipart_length)
                    .position(|w| &w == multipart_delimiter)
                {
                    let form_data_binaries = &multipart[..next_form_data_start_index];
                    new_file.decode_from_bytes(form_data_binaries.to_vec())?;
                    request.files.push(new_file);
                    multipart = &multipart[next_form_data_start_index + multipart_length..];
                } else {
                    let form_data_binaries = &multipart;
                    new_file.decode_from_bytes(form_data_binaries.to_vec())?;
                    request.files.push(new_file);
                    break;
                }
            }
            dbg!(&request);
        }

        Ok(request)
    }

    pub fn from(request_string: &String) -> Result<HttpRequest, PpdcError> {
        let mut request_lines_iterator = request_string.split("\r\n");
        let mut request = HttpRequest::new("", "", "");

        let method_line = request_lines_iterator.next().ok_or_else(|| {
            return PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Request is empty : {:#?}", request_string),
            );
        })?;

        request.parse_request_first_line(method_line)?;

        for row in &mut request_lines_iterator {
            if row == "" {
                break;
            }
            let row = row.split(": ").collect::<Vec<&str>>();
            request
                .headers
                .insert(row[0].to_string().to_lowercase(), row[1].to_string());
        }

        if let Some(content_type) = request.headers.get("content-type") {
            if *&content_type.len() >= 19 && &content_type[..19] == "multipart/form-data" {
                request.content_type = Some(ContentType::FormData);
                request.delimiter =
                    Some(content_type.split("boundary=").collect::<Vec<&str>>()[1].to_string());
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

    pub fn set_received_at(&mut self, received_at: NaiveDateTime) -> () {
        self.received_at = Some(received_at);
    }

    pub fn parsed_path(&self) -> Vec<&str> {
        self.path[1..].split("/").collect::<Vec<&str>>()
    }

    pub fn parse_uuid(uuid: &str) -> Result<Uuid, PpdcError> {
        Uuid::parse_str(uuid).map_err(|err| {
            PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Incorrect uuid for ressource : {:#?}", err),
            )
        })
    }

    pub fn get_pagination(&self) -> (i64, i64) {
        let offset: i64 = self
            .query
            .get("offset")
            .unwrap_or(&"0".to_string())
            .parse()
            .unwrap();
        let limit: i64 = self
            .query
            .get("limit")
            .unwrap_or(&"20".to_string())
            .parse()
            .unwrap();
        (offset, limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REQUEST_STRING: &str = "POST /users HTTP/1.1\r\nHost: 127.0.0.1:8080\r\nOrigin: http://localhost:5173\r\n\r\n{\"name\":\"mathieu\"}";
    fn create_example_hashmap() -> HashMap<String, String> {
        let mut hashmap = HashMap::new();
        hashmap.insert("host".to_string(), "127.0.0.1:8080".to_string());
        hashmap.insert("origin".to_string(), "http://localhost:5173".to_string());
        hashmap
    }

    #[test]
    fn test_request_encoding() {
        let request_bytes = REQUEST_STRING.as_bytes();
        let request = HttpRequest::from_bytes(request_bytes.to_vec()).unwrap();

        let expected_headers = create_example_hashmap();
        assert_eq!(request.method, "POST".to_string());
        assert_eq!(request.path, "/users".to_string());
        assert_eq!(request.headers, expected_headers);
    }

    #[test]
    fn parse_header_from_string() {
        let header_string = "Host: 127.0.0.1:8080\r\nOrigin: http://localhost:5173".to_string();
        let expected_headers = create_example_hashmap();
        let mut request = HttpRequest::new("", "", "");
        request.parse_headers(header_string).unwrap();
        assert_eq!(request.headers, expected_headers);
    }

    #[test]
    fn parse_empty_request() {
        let empty_request_string = "";
        let request_bytes = empty_request_string.as_bytes();
        let request = HttpRequest::from_bytes(request_bytes.to_vec());
        match request {
            Err(_) => (),
            Ok(_) => panic!("should be an error"),
        }
    }

    #[test]
    fn parse_request_with_no_body() {
        let no_body_request_string: &str = "POST /users HTTP/1.1\r\n\r\n";
        let _request = HttpRequest::from_bytes(no_body_request_string.as_bytes().to_vec()).unwrap();
    }
}
