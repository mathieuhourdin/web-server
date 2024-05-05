use crate::entities::error::PpdcError;
use std::collections::HashMap;

#[derive(Debug)]
pub struct File {
    pub name: String,
    pub headers: HashMap<String, String>,
    pub file_type: Option<String>,
    pub content: String
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

