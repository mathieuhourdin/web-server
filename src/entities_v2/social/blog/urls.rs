use uuid::Uuid;

pub(super) fn base() -> String {
    std::env::var("BLOG_BASE_URL")
        .unwrap_or_else(|_| "/blog".to_owned())
        .trim_end_matches('/')
        .to_owned()
}

pub(super) fn publication(slug: &str, id: Uuid) -> String {
    format!("{}/{slug}/posts/{id}", base())
}

pub(super) fn short(code: &str) -> String {
    format!("{}/p/{code}", base())
}

pub(super) fn valid_short_code(code: &str) -> bool {
    code.len() == 8
        && code
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
