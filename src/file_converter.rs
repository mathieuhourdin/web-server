use crate::entities::error::PpdcError;
use regex::Regex;
use encoding_rs::WINDOWS_1252;
use crate::http::HttpRequest;

pub fn convert_tex_file(tex_file: String) -> Result<String, PpdcError> {
    println!("{:?}", tex_file);
    let tex_file = tex_file.replace("\n\n", "\u{0000}");
    let tex_file = tex_file.replace("\n", " ");
    let tex_file = tex_file.replace("\u{0000}", "\n");
    let tex_file = tex_file.replace("\\bigkip", "\n\n");
    let tex_file = tex_file.replace("\\\"{\\i}", "ï");
    let tex_file = tex_file.replace("{\\oe}", "œ");
    let tex_file = tex_file.replace("$\\%$", "%");

    let regex_italic = Regex::new(r"\{\s*\\em\s+(.*?)\s*\}").unwrap();
    let tex_file = regex_italic.replace_all(tex_file.as_str(), |caps: &regex::Captures| {
        format!("{}", &caps[1])
    }).to_string();

    let regex_subsection = Regex::new(r"\\subsection\*\{([^}]*)\}").unwrap();
    let tex_file = regex_subsection.replace_all(tex_file.as_str(), |caps: &regex::Captures| {
        format!("/n/n#{}\n", &caps[1])
    }).to_string();
    

    Ok(tex_file)
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
