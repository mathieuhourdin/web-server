use crate::entities::error::{self, PpdcError};
use std::collections::HashMap;
use regex::Regex;
use encoding_rs::WINDOWS_1252;
use crate::http::{self, HttpRequest, HttpResponse};
use std::fmt;
use serde::{Serialize, Deserialize};

pub struct File {
    headers: HashMap<String, String>,
    file_type: Option<String>,
    content: String
}

#[derive(PartialEq, Serialize, Deserialize)]
pub struct FileConversion {
    file_name: String,
    file_type: Option<String>,
    content: Option<String>
}

impl FileConversion {
    pub fn new(file_name: String, file_type: Option<String>, content: Option<String>) -> FileConversion {
        FileConversion { file_name, file_type, content }
    }
}

impl File {
    pub fn new() -> File {
        File {
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

        for row in splitted_content {
            self.content += row;
            self.content += "\r\n";
        }
        
        Ok(())
    }
}

pub fn replace_tex_marker_by_line_header(text: String, regex: &str, header: &str) -> Result<String, PpdcError> {
    let regex_italic = Regex::new(regex).unwrap();
    Ok(regex_italic.replace_all(text.as_str(), |caps: &regex::Captures| {
        format!("{}{}", header, &caps[1])
    }).to_string())
}

pub fn convert_tex_file(tex_file: String) -> Result<String, PpdcError> {
    println!("{:?}", tex_file);
    let document_begin = "\\begin{document}";
    let document_end = "\\end{document}";

    let document_content;
    if let Some(start) = tex_file.find(document_begin) {
        let content_start_index = start + document_begin.len();
        if let Some(end) = tex_file.find(document_end) {
            let content_end_index = end;
            document_content = &tex_file[content_start_index..content_end_index];
        } else {
            return Err(PpdcError::new(404, error::ErrorType::ApiError, "No document end".to_string()));
        }
    } else {
        return Err(PpdcError::new(404, error::ErrorType::ApiError, "No document start".to_string()));
    }
    let document_content = document_content.replace("\n\n", "\u{0000}");
    let document_content = document_content.replace("\n", " ");
    let document_content = document_content.replace("\u{0000}", "\n");
    let document_content = document_content.replace("\\bigskip", "\n\n");
    let document_content = document_content.replace("\\\"{\\i}", "ï");
    let document_content = document_content.replace("{\\oe}", "œ");
    let document_content = document_content.replace("$\\%$", "%");


    //italic replacement. Nothing for now
    let document_content = replace_tex_marker_by_line_header(document_content, r"\{\s*\\em\s+(.*?)\s*\}", "").unwrap();

    let document_content = replace_tex_marker_by_line_header(document_content, r"\\subsection\*\{([^}]*)\}", "\n\n#").unwrap();
    let document_content = replace_tex_marker_by_line_header(document_content, r"\{\s*\\flushleft\s+(.*?)\s*\}", "").unwrap();
    let document_content = replace_tex_marker_by_line_header(document_content, r"\{\s*\\large\\bf\s+(.*?)\s*", "").unwrap();

    Ok(document_content)
}

pub fn parse_multipart_form_data_request_body(request: &HttpRequest) -> Result<String, PpdcError> {
    if request.content_type.as_ref().unwrap() != &http::ContentType::FormData {
        return Err(PpdcError::new(404, error::ErrorType::ApiError, "Not multipart/form-data content type".to_string()));
    }
    let parts_array = request.body.split(request.delimiter.as_ref().unwrap().as_str()).collect::<Vec<&str>>();
    println!("Parts array : {:?}", parts_array);
    let mut file = File::new();
    file.decode_file(parts_array[1]).unwrap();
    
    convert_tex_file(file.content)
}

pub fn post_file_conversion_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let file = parse_multipart_form_data_request_body(request)?;
    let response = FileConversion::new("title".to_string(), None, Some(file));
    HttpResponse::ok()
        .json(&response)
}

pub fn parse_form_data_request(request: &HttpRequest) -> Result<String, PpdcError> {
    let content_type_header = request.headers.get("content-type").unwrap();
    println!("Content type header : {:?}", content_type_header);
    
    let splitted_content = content_type_header.split(";").collect::<Vec<&str>>();

    let is_form_data = splitted_content[0] == "multipart/form-data";
    println!("Is form data : {:?}", is_form_data);

    let boundary = content_type_header.split("boundary=").collect::<Vec<&str>>()[1];
    println!("Boundary : {:?}", boundary);

    let file_part = request.body.split(boundary).collect::<Vec<&str>>()[1];
    println!("File part : {:?}", file_part);

    Ok(String::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::io::{self, Read, BufReader};

    fn decode_form_data() {
        let request_txt = fs::read_to_string("test_data/form_data_request.txt").unwrap();
        let request_txt = request_txt.replace("\n", "\r\n");

        let request = HttpRequest::from(&request_txt).unwrap();

        println!("Request body : {:?}", request.body);

        let form_data = parse_form_data_request(&request).unwrap();

        let expected_result_file = fs::read_to_string("test_data/tex_example.tex").unwrap();

        assert_eq!(form_data, expected_result_file);
    }

    fn compare_files() {
        let tex_file = fs::File::open("test_data/tex_example.tex").unwrap();
        let mut reader = BufReader::new(tex_file);
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        let (cow, _, had_errors) = WINDOWS_1252.decode(&bytes);

        //let tex_file_string = String::from_utf8_lossy(&tex_file_utf8);
        let tex_file_string = cow;

        println!("Initial file {:?}", tex_file_string);
        let expected_result_file = fs::read_to_string("test_data/tex_example_expected_result.txt").unwrap();
        println!("Expected result {:?}", expected_result_file);
        
        let converted_file = convert_tex_file(tex_file_string.to_string()).unwrap();
        //let mut file = fs::File::create("test_data/ps.txt").unwrap();
        //file.write_all(converted_file.as_bytes());
        
        let converted_file = converted_file[1..converted_file.len() -1].to_string();
        assert_eq!( "\n".to_owned() + &converted_file + "\n", expected_result_file);
    }

}
