use scraper::{Html,Selector};
use crate::entities::resource::resource_type::ResourceType;

pub fn parse_page_title(html: &Html) -> Option<String> {
    if let Some(title) = find_opengraph_content_from_property(html, "title") {
        println!("parse_page_title : OG Title : {}", title);
        return Some(title);
    }
    if let Some(title) = find_twitter_content_from_name(html, "title") {
        println!("parse_page_title : Twitter Title : {}", title);
        return Some(title);
    }
    if let Some(title) = find_schema_content_from_itemprop(html, "name") {
        println!("parse_page_title : Schema Title : {}", title);
        return Some(title);
    }
    if let Some(title) = find_raw_html_title(html) {
        println!("parse_page_title : Raw Html Title : {}", title);
        return Some(title);
    }
    None
}

pub fn find_raw_html_title(html: &Html) -> Option<String> {
    let selector = Selector::parse("title").unwrap();
    html.select(&selector).map(|e| e.inner_html()).next()
}

pub fn find_opengraph_content_from_property(html: &Html, property: &str) -> Option<String> {
    let selector = Selector::parse(&format!("meta[property=\"og:{}\"]", property)).unwrap();
    html.select(&selector).map(|e| e.attr("content").unwrap().to_string()).next()
}

pub fn find_twitter_content_from_name(html: &Html, name: &str) -> Option<String> {
    let selector = Selector::parse(&format!("meta[name=\"twitter:{}\"]", name)).unwrap();
    html.select(&selector).map(|e| e.attr("content").unwrap().to_string()).next()
}

pub fn find_schema_content_from_itemprop(html: &Html, prop: &str) -> Option<String> {
    let selector = Selector::parse(&format!("meta[itemprop=\"{}\"]", prop)).unwrap();
    html.select(&selector).map(|e| e.attr("content").unwrap().to_string()).next()
}

pub fn parse_page_subtitle(html: &Html) -> Option<String> {
    if let Some(subtitle) = find_opengraph_content_from_property(html, "description") {
        return Some(subtitle);
    }
    if let Some(subtitle) = find_twitter_content_from_name(html, "description") {
        return Some(subtitle)
    }
    if let Some(subtitle) = find_schema_content_from_itemprop(html, "description") {
        return Some(subtitle)
    }
    None
}

pub fn parse_page_image_url(html: &Html) -> Option<String> {
    if let Some(image_url) = find_opengraph_content_from_property(html, "image") {
        return Some(image_url);
    }
    if let Some(image_url) = find_twitter_content_from_name(html, "image") {
        return Some(image_url);
    }
    if let Some(image_url) = find_schema_content_from_itemprop(html, "image") {
        return Some(image_url);
    }
    if let Some(image_url) = find_raw_html_image_url(html) {
        return Some(image_url);
    }
    None
}

pub fn find_raw_html_image_url(html: &Html) -> Option<String> {
    let selector = Selector::parse("img").unwrap();
    html.select(&selector).map(|e| e.attr("src").unwrap().to_string()).next()
}

pub fn parse_content(_html: &Html) -> Option<String> {
    None
}

pub fn parse_resource_type(html: &Html) -> Option<String> {
    if let Some(resource_type) = find_opengraph_content_from_property(html, "type") {
        if let Some(resource_type_enum) = ResourceType::from_opengraph_code(resource_type.as_str()) {
            return Some(resource_type_enum.to_code().to_string());
        } else {
            return Some(resource_type);
        }
    }
    None
}

#[cfg(test)]
mod tests {

    use super::*;
    use tokio::test;
    use std::fs;

    fn get_structured_data_html() -> Html {
        Html::parse_document(&fs::read_to_string("test_data/structured_html.html").unwrap())
    }
    fn get_opengraph_data_html() -> Html {
        Html::parse_document(&fs::read_to_string("test_data/open_graph.html").unwrap())
    }
    fn get_twitter_data_html() -> Html {
        Html::parse_document(&fs::read_to_string("test_data/twitter.html").unwrap())
    }
    fn get_schema_data_html() -> Html {
        Html::parse_document(&fs::read_to_string("test_data/schema.html").unwrap())
    }
    fn get_raw_html_data_html() -> Html {
        Html::parse_document(&fs::read_to_string("test_data/raw_html.html").unwrap())
    }


    #[test]
    async fn test_parse_page_title() {
        let example_html = get_structured_data_html();
        let parse_result = parse_page_title(&example_html).unwrap();
        assert_eq!(parse_result, "og_title".to_string())
    }

    #[test]
    async fn test_parse_page_title_opengraph() {
        let opengraph_html = get_opengraph_data_html();
        let parse_result = parse_page_title(&opengraph_html).unwrap();
        assert_eq!(parse_result, "og_title".to_string())
    }
    #[test]
    async fn test_parse_page_title_twitter() {
        let twitter_html = get_twitter_data_html();
        let parse_result = parse_page_title(&twitter_html).unwrap();
        assert_eq!(parse_result, "twitter_title".to_string())
    }
    #[test]
    async fn test_parse_page_title_schema() {
        let schema_html = get_schema_data_html();
        let parse_result = parse_page_title(&schema_html).unwrap();
        assert_eq!(parse_result, "schema_title".to_string())
    }

    #[test]
    async fn test_find_raw_html_title() {
        let example_html = get_structured_data_html();
        let parse_result = find_raw_html_title(&example_html).unwrap();
        assert_eq!(parse_result, "raw_html_title".to_string())
    }

    #[test]
    async fn test_find_opengraph_title() {
        let example_html = get_structured_data_html();
        let parse_result = find_opengraph_content_from_property(&example_html, "title").unwrap();
        assert_eq!(parse_result, "og_title".to_string())
    }

    #[test]
    async fn test_find_twitter_title() {
        let example_html = get_structured_data_html();
        let parse_result = find_twitter_content_from_name(&example_html, "title").unwrap();
        assert_eq!(parse_result, "twitter_title".to_string())
    }

    #[test]
    async fn test_find_schema_title() {
        let example_html = get_structured_data_html();
        let parse_result = find_schema_content_from_itemprop(&example_html, "name").unwrap();
        assert_eq!(parse_result, "schema_title".to_string())
    }

    #[test]
    async fn test_parse_page_subtitle_opengraph() {
        let example_html = get_opengraph_data_html();
        let parse_result = parse_page_subtitle(&example_html).unwrap();
        assert_eq!(parse_result, "og_description".to_string());
    }
    #[test]
    async fn test_parse_page_subtitle_twitter() {
        let example_html = get_twitter_data_html();
        let parse_result = parse_page_subtitle(&example_html).unwrap();
        assert_eq!(parse_result, "twitter_description".to_string());
    }

    #[test]
    async fn test_parse_page_subtitle_schema() {
        let example_html = get_schema_data_html();
        let parse_result = parse_page_subtitle(&example_html).expect("should have a value");
        assert_eq!(parse_result, "schema_description".to_string());
    }

    #[test]
    async fn test_find_opengraph_description() {
        let example_html = get_structured_data_html();
        let parse_result = find_opengraph_content_from_property(&example_html, "description").unwrap();
        assert_eq!(parse_result, "og_description".to_string())
    }

    #[test]
    async fn test_find_twitter_description() {
        let example_html = get_structured_data_html();
        let parse_result = find_twitter_content_from_name(&example_html, "description").unwrap();
        assert_eq!(parse_result, "twitter_description".to_string())
    }

    #[test]
    async fn test_find_schema_description() {
        let example_html = get_structured_data_html();
        let parse_result = find_schema_content_from_itemprop(&example_html, "description").unwrap();
        assert_eq!(parse_result, "schema_description".to_string())
    }

    #[test]
    async fn test_parse_page_image_url_opengraph() {
        let example_html = get_opengraph_data_html();
        let parse_result = parse_page_image_url(&example_html).expect("Should have a value");
        assert_eq!(parse_result, "og_image".to_string());
    }

    #[test]
    async fn test_parse_page_image_url_twitter() {
        let example_html = get_twitter_data_html();
        let parse_result = parse_page_image_url(&example_html).expect("Should have a value");
        assert_eq!(parse_result, "twitter_image".to_string());
    }

    #[test]
    async fn test_parse_page_image_url_schema() {
        let example_html = get_schema_data_html();
        let parse_result = parse_page_image_url(&example_html).expect("Should have a value");
        assert_eq!(parse_result, "schema_image".to_string());
    }

    #[test]
    async fn test_parse_page_image_url_raw_html() {
        let example_html = get_raw_html_data_html();
        let parse_result = parse_page_image_url(&example_html).expect("Should have a value");
        assert_eq!(parse_result, "raw_html_image".to_string());
    }

    #[test]
    async fn test_find_raw_html_image_url() {
        let example_html = get_structured_data_html();
        let parse_result = find_raw_html_image_url(&example_html).unwrap();
        assert_eq!(parse_result, "raw_html_image".to_string())
    }

    #[test]
    async fn test_find_opengraph_image_url() {
        let example_html = get_structured_data_html();
        let parse_result = find_opengraph_content_from_property(&example_html, "image").unwrap();
        assert_eq!(parse_result, "og_image".to_string())
    }

    #[test]
    async fn test_find_twitter_image_url() {
        let example_html = get_structured_data_html();
        let parse_result = find_twitter_content_from_name(&example_html, "image").unwrap();
        assert_eq!(parse_result, "twitter_image".to_string())
    }

    #[test]
    async fn test_find_schema_image_url() {
        let example_html = get_structured_data_html();
        let parse_result = find_schema_content_from_itemprop(&example_html, "image").unwrap();
        assert_eq!(parse_result, "schema_image".to_string())
    }
}
