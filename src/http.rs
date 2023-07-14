use std::collections::HashMap;

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
        HttpResponse { status_code, headers, body }
    }

    pub fn to_stream(&self) -> String {
        let status_line = self.status_code.to_status_line();
        let content_size = self.body.len();
        format!("{status_line}\r\nContent-Length: {content_size}\r\n\r\n{}", self.body)
    }
}

pub enum StatusCode {
    Ok,
    NotFound,
    Created,
    BadRequest,
}

impl StatusCode {
    pub fn to_status_line(&self) -> &str {
        match self {
            StatusCode::Ok => "HTTP/1.1 200 OK",
            StatusCode::Created => "HTTP/1.1 201 CREATED",
            StatusCode::NotFound => "HTTP/1.1 404 NOT FOUND",
            StatusCode::BadRequest => "HTTP/1.1 400 BAD REQUEST",
        }
    }
}
