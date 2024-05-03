use super::*;
use crate::environment::get_api_url;
use serde::Serialize;
use serde_json;

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

