use crate::entities::error::{self, PpdcError};
use std::collections::HashMap;
use crate::http::{self, HttpRequest, HttpResponse};
use serde::{Serialize, Deserialize};

mod tex;
mod docx_to_spip;


#[derive(Debug)]
pub struct File {
    name: String,
    headers: HashMap<String, String>,
    file_type: Option<String>,
    content: String
}

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

impl File {
    pub fn new() -> File {
        File {
            name: String::new(),
            headers: HashMap::new(),
            file_type: None,
            content: String::new()
        }
    }

    pub fn decode_file(&mut self, file_str: &str) -> Result<(), PpdcError> {

        let mut splitted_content = file_str.split("\r\n");
        splitted_content.next();
        for row in &mut splitted_content {
            if row == "" {
                break;
            }
            let row = row.split(": ").collect::<Vec<&str>>();
            self.headers.insert(row[0].to_string().to_lowercase(), row[1].to_string());
        }

        if let Some(content_type) = self.headers.get("content-type") {
            self.file_type = Some(content_type.to_string());
        }
        if let Some(content_disposition) = self.headers.get("content-disposition") {
            for subheader in content_disposition.split("; ") {
                let mut splitted_subheader = subheader.split("=");
                if let Some(key) = splitted_subheader.next() {
                    if key == "name" {
                        let name = splitted_subheader.next().unwrap();
                        let name = &name[1..name.len() -1];
                        self.name = name.to_string();
                    }
                }
            }
        }
        for row in splitted_content {
            self.content += row;
            self.content += "\r\n";
        }
        
        Ok(())
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
