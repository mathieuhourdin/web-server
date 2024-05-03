use crate::entities::error::{self, PpdcError};
use regex::Regex;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::{Read, BufReader};
    use encoding_rs::WINDOWS_1252;

    fn compare_files() {
        let tex_file = fs::File::open("test_data/tex_example.tex").unwrap();
        let mut reader = BufReader::new(tex_file);
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        let (cow, _, _) = WINDOWS_1252.decode(&bytes);

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
