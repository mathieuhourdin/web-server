use crate::http::{HttpRequest, HttpResponse, StatusCode};
use serde::{Serialize, Deserialize};
use crate::entities::error::PpdcError;
use reqwest;
use std::fs;
use scraper::{Html,Selector};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct NewLinkPreview {
    pub external_content_url: String
}

#[derive(Debug, PartialEq, Serialize)]
pub struct LinkPreview {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub external_content_url: String,
    pub image_url: Option<String>,
    pub content: Option<String>
}

pub async fn post_preview_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    println!("link_preview");
    let decoded_request = decode_request_body(&request.body[..]).unwrap();
    let response = generate_link_preview(decoded_request.external_content_url).await.unwrap();
    HttpResponse::ok()
        .json(&response)

}

pub fn decode_request_body(request_body: &str) -> Result<NewLinkPreview, PpdcError> {
    Ok(serde_json::from_str::<NewLinkPreview>(request_body)?)
}

pub async fn generate_link_preview(link: String) -> Result<LinkPreview, PpdcError> {
    let html_link = fetch_external_resource_html_body(&link).await?;
    let parsed_html = parse_html(html_link);
    Ok(generate_preview_from_html(parsed_html, link))
}

pub fn parse_html(html_file: String) -> Html {
    Html::parse_document(&html_file[..])
}

pub fn generate_preview_from_html(html: Html, external_content_url: String) -> LinkPreview {
    LinkPreview {
        title: parse_page_title(&html),
        subtitle: parse_page_subtitle(&html),
        external_content_url,
        image_url: parse_page_image_url(&html),
        content: parse_content(&html)
    }
}

pub fn parse_page_title(html: &Html) -> Option<String> {
    let selector = Selector::parse("title").unwrap();
    html.select(&selector).map(|e| e.inner_html()).next()
}

pub fn parse_page_image_url(html: &Html) -> Option<String> {
    None
}

pub fn parse_page_subtitle(html: &Html) -> Option<String> {
    None
}

pub fn parse_content(html: &Html) -> Option<String> {
    None
}

pub async fn fetch_external_resource_html_body(external_resource_url: &str) -> Result<String, PpdcError> {
    println!("{}", external_resource_url);
    let response = reqwest::get(external_resource_url).await.unwrap();
    let response = response.text().await.unwrap();
    println!("{}", response);
    Ok(response)
}


#[cfg(test)]
mod tests {

    use super::*;
    use tokio::test;

    #[test]
    async fn test_post_review_route_works() {
        let request = HttpRequest::new("POST", "/link_preview", "{
            \"external_content_url\": \"https://example.com\"
        }");
        let expected_response = HttpResponse::new(StatusCode::Ok, "{\"title\":\"Example Domain\",\"subtitle\":null,\"external_content_url\":\"https://example.com\",\"image_url\":null,\"content\":null}".to_string());
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
            content: None
        };
        assert_eq!(generate_preview_from_html(parse_result, "https://example.com".to_string()), link_preview);
    }

    #[test]
    async fn test_parse_page_title() {
        let example_page = fs::read_to_string("test_data/example.html").unwrap();
        let parse_result = parse_page_title(&Html::parse_document(&example_page)).unwrap();
        assert_eq!(parse_result, "Example Domain".to_string())
    }

    #[test]
    async fn test_generate_link_preview() {
        let expected_response: LinkPreview = LinkPreview {
            title: Some("Example Domain".to_string()),
            subtitle: None,
            external_content_url: "https://example.com".to_string(),
            image_url: None,
            content: None
        };;
        let response = generate_link_preview("https://example.com".to_string()).await.unwrap();
        assert_eq!(response, expected_response);

    }

    #[tokio::test]
    async fn test_external_resource_fetching() {
        let external_resource_url = String::from("http://example.com/");
        let html_output = fetch_external_resource_html_body(&external_resource_url).await.unwrap();
        let test_html = fs::read_to_string("test_data/example.html").unwrap();
        assert_eq!(html_output, test_html);
    }
}
