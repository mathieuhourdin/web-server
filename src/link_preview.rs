use crate::entities::error::PpdcError;
use axum::{debug_handler, extract::Json};
use reqwest::{header, Client};
use scraper::Html;
use serde::{Deserialize, Serialize};

mod html_parser;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct NewLinkPreview {
    pub external_content_url: String,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct LinkPreview {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub external_content_url: String,
    pub image_url: Option<String>,
    pub content: Option<String>,
    pub resource_type: Option<String>,
}

#[debug_handler]
pub async fn post_preview_route(
    Json(payload): Json<NewLinkPreview>,
) -> Result<Json<LinkPreview>, PpdcError> {
    let response = generate_link_preview(payload.external_content_url).await?;
    Ok(Json(response))
}

pub fn decode_request_body(request_body: &str) -> Result<NewLinkPreview, PpdcError> {
    Ok(serde_json::from_str::<NewLinkPreview>(request_body)?)
}

pub async fn generate_link_preview(link: String) -> Result<LinkPreview, PpdcError> {
    let html_link = fetch_external_resource_html_body(&link).await?;
    let parsed_html = parse_html(html_link);
    Ok(generate_preview_from_html(parsed_html, link))
}

pub async fn fetch_external_resource_html_body(
    external_resource_url: &str,
) -> Result<String, PpdcError> {
    println!("{}", external_resource_url);
    let client = Client::new();

    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::ACCEPT,
        "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8"
            .parse()
            .unwrap(),
    );
    let response = client
        .get(external_resource_url)
        .headers(headers)
        .send()
        .await
        .unwrap();
    let response = response.text().await.unwrap();
    println!("{}", response);
    Ok(response)
}

pub fn parse_html(html_file: String) -> Html {
    Html::parse_document(&html_file[..])
}

pub fn generate_preview_from_html(html: Html, external_content_url: String) -> LinkPreview {
    LinkPreview {
        title: html_parser::parse_page_title(&html),
        subtitle: html_parser::parse_page_subtitle(&html),
        external_content_url,
        image_url: html_parser::parse_page_image_url(&html),
        content: html_parser::parse_content(&html),
        resource_type: html_parser::parse_resource_type(&html),
    }
}

#[cfg(test)]
mod tests {

    /*
    use super::*;
    use tokio::test;
    use std::fs;
    use crate::http::StatusCode;

    #[test]
    async fn test_post_review_route_works() {
        let request = HttpRequest::new("POST", "/link_preview", "{
            \"external_content_url\": \"https://example.com\"
        }");
        let expected_response = HttpResponse::new(StatusCode::Ok, "{\"title\":\"Example Domain\",\"subtitle\":null,\"external_content_url\":\"https://example.com\",\"image_url\":null,\"content\":null,\"resource_type\":null}".to_string());
        let response = post_preview_route(&request).await.unwrap();
        assert_eq!(expected_response, response);
    }

    #[test]
    async fn decode_post_request_body() {
        let test_request_body = "{
            \"external_content_url\": \"https://example.com\"
        }".to_string();
        let expected_new_link_preview = NewLinkPreview { external_content_url: String::from("https://example.com") };
        let function_result = decode_request_body(test_request_body.as_str())
            .unwrap();
        assert_eq!(function_result, expected_new_link_preview)
    }

    #[test]
    async fn test_generate_preview_from_html() {
        let example_page = fs::read_to_string("test_data/example.html").unwrap();
        let parse_result = Html::parse_document(&example_page);
        let link_preview = LinkPreview {
            title: Some("Example Domain".to_string()),
            subtitle: None,
            external_content_url: "https://example.com".to_string(),
            image_url: None,
            content: None,
            resource_type: None
        };
        assert_eq!(generate_preview_from_html(parse_result, "https://example.com".to_string()), link_preview);
    }

    #[test]
    async fn test_generate_link_preview() {
        let expected_response: LinkPreview = LinkPreview {
            title: Some("Example Domain".to_string()),
            subtitle: None,
            external_content_url: "https://example.com".to_string(),
            image_url: None,
            content: None,
            resource_type: None
        };
        let response = generate_link_preview("https://example.com".to_string()).await.unwrap();
        assert_eq!(response, expected_response);
    }

    #[tokio::test]
    async fn test_external_resource_fetching() {
        let external_resource_url = String::from("http://example.com/");
        let html_output = fetch_external_resource_html_body(&external_resource_url).await.unwrap();
        let test_html = fs::read_to_string("test_data/example.html").unwrap();
        assert_eq!(html_output, test_html);
    }*/
}
