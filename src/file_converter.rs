use crate::entities::error::{self, PpdcError};
use crate::http::{self, HttpRequest, HttpResponse};
use serde::{Serialize, Deserialize};
use crate::http::File;

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
    let parts_array = request.body.split(request.delimiter.as_ref().unwrap().as_str()).collect::<Vec<&str>>();
    println!("Parts array : {:?}", parts_array);
    let mut file = File::new();
    file.decode_file(parts_array[1]).unwrap();

    println!("{:?}", file);
    
    file.content = tex::convert_tex_file(file.content).unwrap();
    Ok(file)
}

pub fn post_file_conversion_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let file = parse_multipart_form_data_request_body(request)?;
    let response = FileConversion::new(file.name, file.file_type, Some(file.content));
    HttpResponse::ok()
        .json(&response)
}

#[cfg(test)]
mod tests {
    //use super::*;
}
