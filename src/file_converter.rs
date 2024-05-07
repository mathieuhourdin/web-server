use crate::entities::error::{self, PpdcError};
use crate::http::{self, HttpRequest, HttpResponse};
use serde::{Serialize, Deserialize};
use crate::http::{File, FileType};

mod tex;
mod docx_to_spip;

#[derive(PartialEq, Serialize, Deserialize)]
pub struct FileConversion {
    name: String,
    file_type: Option<String>,
    content: Option<String>
}

impl FileConversion {
    pub fn new(name: String, file_type: Option<String>, content: Option<String>) -> FileConversion {
        FileConversion { name, file_type, content }
    }
}

pub fn parse_multipart_form_data_request_body(request: &HttpRequest) -> Result<File, PpdcError> {
    if request.content_type.as_ref().unwrap() != &http::ContentType::FormData {
        return Err(PpdcError::new(404, error::ErrorType::ApiError, "Not multipart/form-data content type".to_string()));
    }
    let target_format: String = request.query.get("target_format").unwrap_or(&"ppdc".to_string()).to_string();
    let mut file = request.files[0].clone();

    if file.file_type == Some(FileType::Docx) {
        println!("This is a docx file");
    }
    if &target_format == "ppdc" && file.file_type == Some(FileType::Tex) {
        file.content = tex::convert_tex_file(file.content)?;
    } else if &target_format == "spip" && file.file_type == Some(FileType::Docx) {
        file.content = docx_to_spip::handle_file_conversion(&file);
    }
    Ok(file)
}

pub fn post_file_conversion_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let file = parse_multipart_form_data_request_body(request)?;
    let response = FileConversion::new(file.name, None, Some(file.content));
    HttpResponse::ok()
        .json(&response)
}

#[cfg(test)]
mod tests {
    //use super::*;
}
