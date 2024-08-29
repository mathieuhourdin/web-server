use crate::entities::error::{self, PpdcError};
use crate::http::{self, HttpRequest, HttpResponse};
use serde::{Serialize, Deserialize};
use crate::http::{File, FileType};
use axum::{debug_handler, extract::{Json, Query, Multipart}};

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

#[derive(Deserialize)]
pub struct FileConversionParams {
    target_format: Option<String>,
    remove_line_breaks: Option<bool>
}

impl FileConversionParams {
    pub fn target_format(&self) -> String {
        self.target_format.clone().unwrap_or("ppdc".to_string())
    }

    pub fn remove_line_breaks(&self) -> bool {
        self.remove_line_breaks.clone().unwrap_or(true)
    }
}

#[debug_handler]
pub async fn post_file_conversion_route(
    Query(params): Query<FileConversionParams>,
    mut multipart: Multipart
) -> Result<Json<FileConversion>, PpdcError> {

    let field = multipart.next_field().await.unwrap().unwrap();
    let mut file = File::new();
    file.from_axum_field(field).await;

    if file.file_type == Some(FileType::Docx) {
        println!("This is a docx file");
    }
    if params.target_format() == "ppdc" && file.file_type == Some(FileType::Tex) {
        file.content = tex::convert_tex_file(file.content)?;
    } else if params.target_format() == "spip" && file.file_type == Some(FileType::Docx) {
        file.content = docx_to_spip::handle_file_conversion(&file, params.remove_line_breaks());
    }
    let response = FileConversion::new(file.name, None, Some(file.content));
    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    //use super::*;
}
