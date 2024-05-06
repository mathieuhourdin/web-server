use crate::entities::error::PpdcError;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone)]
pub enum FileType {
    Tex,
    Text,
    Docx
}

impl FileType {
    pub fn from_str(type_str: &str) -> Option<FileType> {
        match type_str {
            "tex" => Some(FileType::Tex),
            "txt" => Some(FileType::Text),
            "docx" => Some(FileType::Docx),
            _ => None
        }
    }

    pub fn to_string(self) -> String {
        match self {
            FileType::Tex => "tex",
            FileType::Text => "txt",
            FileType::Docx => "docx"
        }.to_string()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct File {
    pub name: String,
    pub headers: HashMap<String, String>,
    pub file_type: Option<FileType>,
    pub content: String,
    pub content_bytes: Vec<u8>
}

impl File {
    pub fn new() -> File {
        File {
            name: String::new(),
            headers: HashMap::new(),
            file_type: None,
            content: String::new(),
            content_bytes: vec![]
        }
    }

    pub fn decode_from_bytes(&mut self, file_bytes: Vec<u8>) -> Result<(), PpdcError> {

        let content_delimiter = b"\r\n\r\n";
        let content_start_index = file_bytes.windows(4).position(|w| w == content_delimiter).expect("Should have a content");
        let headers_part = file_bytes[..content_start_index].to_vec();
        let headers_part = String::from_utf8(headers_part).unwrap();
        dbg!(&headers_part);


        let splitted_content = headers_part[2..].split("\r\n");
        for row in splitted_content {
            dbg!(row);
            let row = row.split(": ").collect::<Vec<&str>>();
            self.headers.insert(row[0].to_string().to_lowercase(), row[1].to_string());
        }
        if let Some(content_disposition) = self.headers.get("content-disposition") {
            for subheader in content_disposition.split("; ") {
                let mut splitted_subheader = subheader.split("=");
                if let Some(key) = splitted_subheader.next() {
                    if key == "filename" {
                        let name = splitted_subheader.next().unwrap();
                        let name = &name[1..name.len() -1];
                        self.name = name.to_string();
                        self.file_type = name.split(".").last().map(|x| FileType::from_str(x)).unwrap();
                    }
                }
            }
        }

        let content_part = &file_bytes[content_start_index +4..];
        self.content = String::from_utf8_lossy(content_part).to_string();
        self.content_bytes = content_part.to_vec();
         
        Ok(())
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

        if let Some(content_disposition) = self.headers.get("content-disposition") {
            for subheader in content_disposition.split("; ") {
                let mut splitted_subheader = subheader.split("=");
                if let Some(key) = splitted_subheader.next() {
                    if key == "name" {
                        let name = splitted_subheader.next().unwrap();
                        let name = &name[1..name.len() -1];
                        self.file_type = name.split(".").last().map(|x| FileType::from_str(x)).unwrap();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_from_bytes() {
        let example_string = "\r\nContent-Disposition: form-data; name=\"file\"; filename=\"example.txt\"\r\nContent-Type: text/plain\r\n\r\nContent of the file";
        let example_bytes = example_string.as_bytes();

        let mut expected_file = File { 
            name: "example.txt".to_string(),
            headers: HashMap::new(),
            file_type: Some(FileType::Text),
            content: "Content of the file".to_string(),
            content_bytes: b"Content of the file".to_vec()
        };
        expected_file.headers.insert("content-disposition".to_string(), "form-data; name=\"file\"; filename=\"example.txt\"".to_string());
        expected_file.headers.insert("content-type".to_string(), "text/plain".to_string());

        let mut file = File::new();
        file.decode_from_bytes(example_bytes.to_vec());
        assert_eq!(file, expected_file);
    }
}
