use std::time::Duration;

use axum::{debug_handler, extract::Json};
use reqwest::header::{CONTENT_TYPE, USER_AGENT};
use scraper::{Html, Selector};
use url::Url;

use crate::entities_v2::error::{ErrorType, PpdcError};

use super::model::{UrlPreviewRequest, UrlPreviewResponse};

const MAX_PREVIEW_BYTES: usize = 512 * 1024;

#[derive(Debug)]
struct MetaCandidate {
    source_kind: &'static str,
    title: Option<String>,
    description: Option<String>,
    image_url: Option<String>,
}

#[debug_handler]
pub async fn post_url_preview_route(
    Json(payload): Json<UrlPreviewRequest>,
) -> Result<Json<UrlPreviewResponse>, PpdcError> {
    let requested_url = payload.url.trim().to_string();
    let url = validate_preview_url(&requested_url)?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|err| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Failed to build HTTP client: {}", err),
            )
        })?;

    let response = client
        .get(url)
        .header(USER_AGENT, "MatiereGriseUrlPreview/1.0")
        .send()
        .await
        .map_err(|err| {
            PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Failed to fetch URL preview: {}", err),
            )
        })?;

    if !response.status().is_success() {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            format!("Preview URL returned HTTP {}", response.status()),
        ));
    }

    let final_url = response.url().clone();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    if !content_type.is_empty()
        && !content_type.to_ascii_lowercase().contains("text/html")
        && !content_type
            .to_ascii_lowercase()
            .contains("application/xhtml+xml")
    {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Preview URL did not return an HTML document".to_string(),
        ));
    }

    let bytes = response.bytes().await.map_err(|err| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            format!("Failed to read URL preview response: {}", err),
        )
    })?;
    if bytes.len() > MAX_PREVIEW_BYTES {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Preview URL response is too large".to_string(),
        ));
    }

    let html = String::from_utf8_lossy(&bytes).to_string();
    let document = Html::parse_document(&html);

    let og = MetaCandidate {
        source_kind: "opengraph",
        title: meta_property(&document, "og:title"),
        description: meta_property(&document, "og:description"),
        image_url: meta_property(&document, "og:image"),
    };
    let twitter = MetaCandidate {
        source_kind: "twitter_card",
        title: meta_name(&document, "twitter:title"),
        description: meta_name(&document, "twitter:description"),
        image_url: meta_name(&document, "twitter:image"),
    };
    let html_fallback = MetaCandidate {
        source_kind: "html",
        title: html_title(&document),
        description: meta_name(&document, "description"),
        image_url: None,
    };

    let candidate = choose_candidate(og, twitter, html_fallback)?;
    let canonical_url = meta_property(&document, "og:url")
        .or_else(|| canonical_link(&document))
        .and_then(|value| resolve_url(&final_url, &value))
        .unwrap_or_else(|| final_url.to_string());
    let image_url = candidate
        .image_url
        .as_deref()
        .and_then(|value| resolve_url(&final_url, value));
    let site_name = meta_property(&document, "og:site_name");
    let provider_name = site_name
        .clone()
        .unwrap_or_else(|| final_url.host_str().unwrap_or("unknown").to_string());
    let content_type = normalize_content_type(
        meta_property(&document, "og:type")
            .unwrap_or_else(|| "website".to_string())
            .as_str(),
    );

    Ok(Json(UrlPreviewResponse {
        requested_url,
        canonical_url,
        source_kind: candidate.source_kind.to_string(),
        provider_name,
        content_type,
        title: candidate.title.unwrap_or_default(),
        description: candidate.description.unwrap_or_default(),
        image_url,
        site_name,
    }))
}

fn validate_preview_url(raw_url: &str) -> Result<Url, PpdcError> {
    let url = Url::parse(raw_url).map_err(|_| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            "url must be a valid absolute URL".to_string(),
        )
    })?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        _ => Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "url must use http or https".to_string(),
        )),
    }
}

fn choose_candidate(
    og: MetaCandidate,
    twitter: MetaCandidate,
    html_fallback: MetaCandidate,
) -> Result<MetaCandidate, PpdcError> {
    for candidate in [og, twitter, html_fallback] {
        if candidate
            .title
            .as_deref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
            || candidate
                .description
                .as_deref()
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
        {
            return Ok(candidate);
        }
    }
    Err(PpdcError::new(
        400,
        ErrorType::ApiError,
        "No preview metadata found for URL".to_string(),
    ))
}

fn normalize_content_type(og_type: &str) -> String {
    let value = og_type.trim().to_ascii_lowercase();
    if value.starts_with("music.") || value == "music" {
        "audio".to_string()
    } else if value.starts_with("video.") || value == "video" {
        "video".to_string()
    } else if value.starts_with("article") {
        "article".to_string()
    } else if value.is_empty() {
        "unknown".to_string()
    } else {
        value
    }
}

fn meta_property(document: &Html, property: &str) -> Option<String> {
    select_meta_content(document, &format!(r#"meta[property="{}"]"#, property))
}

fn meta_name(document: &Html, name: &str) -> Option<String> {
    select_meta_content(document, &format!(r#"meta[name="{}"]"#, name))
}

fn select_meta_content(document: &Html, selector: &str) -> Option<String> {
    let selector = Selector::parse(selector).ok()?;
    document
        .select(&selector)
        .find_map(|element| element.value().attr("content"))
        .map(clean_text)
        .filter(|value| !value.is_empty())
}

fn html_title(document: &Html) -> Option<String> {
    let selector = Selector::parse("title").ok()?;
    document
        .select(&selector)
        .next()
        .map(|element| clean_text(&element.text().collect::<Vec<_>>().join(" ")))
        .filter(|value| !value.is_empty())
}

fn canonical_link(document: &Html) -> Option<String> {
    let selector = Selector::parse(r#"link[rel="canonical"]"#).ok()?;
    document
        .select(&selector)
        .find_map(|element| element.value().attr("href"))
        .map(clean_text)
        .filter(|value| !value.is_empty())
}

fn resolve_url(base: &Url, value: &str) -> Option<String> {
    base.join(value).ok().map(|url| url.to_string())
}

fn clean_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
