use docx_rs::{Docx, DocumentChild, ParagraphChild, RunChild};
use crate::http::File;

pub fn docx_to_spip(docx: Docx) -> String {
    let mut document_text = String::new();
    for child in docx.document.children {
        //dbg!(&child);
        if let DocumentChild::Paragraph(paragraph) = child {
            let mut last_bold = false;
            for (index, paragraph_child) in paragraph.children.iter().enumerate() {
                if let ParagraphChild::Run(run) = paragraph_child {

                    if run.run_property.bold.is_some() && !last_bold {
                        document_text += "{{";
                        last_bold = true;
                    } else if last_bold && !run.run_property.bold.is_some() {
                        document_text += "}}";
                        last_bold = false;
                    }
                    for run_child in &run.children {
                        if let RunChild::Text(text) = run_child {
                            document_text += &text.text.replace("«", "{«").replace("»", "»}");
                        }
                    }

                    if last_bold && index == paragraph.children.len() -1 {
                        document_text += "}}";
                    }
                }
            }
        }
        document_text += "\n";
        dbg!(&document_text);
    }
    document_text
}

pub fn bytes_to_docx(bytes: &[u8]) -> Docx {
    let docx = docx_rs::read_docx(bytes).unwrap();
    docx
}

pub fn handle_file_conversion(file: &File) -> String {
    let docx = bytes_to_docx(&file.content_bytes);
    docx_to_spip(docx)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use super::*;

    #[test]
    fn test_docx_to_md() {
        let docx_file = fs::read("test_data/fakir_docx_article_example.docx").unwrap();
        let expected_result = "Titre\u{a0}: Les patrons de Sanofi sont des voyous\n\nDepuis longtemps déjà, ils s’en mettent {«\u{a0}plein les poches\u{a0}»}. Nous ne sommes pas d’accord, chez {{Fakir}}.\n\n";

        let docx = docx_rs::read_docx(&docx_file).unwrap();
        let result = docx_to_spip(docx);
        assert_eq!(result, expected_result);
    }
}
