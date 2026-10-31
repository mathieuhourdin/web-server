use super::model::{Blog, BlogPost};
use crate::pagination::PaginatedResponse;
use axum::{
    http::header,
    response::{Html, IntoResponse, Response},
};

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn base() -> String {
    std::env::var("BLOG_BASE_URL")
        .unwrap_or_else(|_| "/blog".to_owned())
        .trim_end_matches('/')
        .to_owned()
}
fn blog_url(blog: &Blog) -> String {
    format!("{}/{}", base(), blog.slug)
}
fn post_url(blog: &Blog, post: &BlogPost) -> String {
    format!("{}/posts/{}", blog_url(blog), post.id)
}
fn image_url(blog: &Blog, post: Option<&BlogPost>, asset: uuid::Uuid) -> String {
    match post {
        Some(p) => format!("{}/assets/{asset}", post_url(blog, p)),
        None => format!("{}/assets/{asset}", blog_url(blog)),
    }
}
fn page(
    blog: &Blog,
    title: &str,
    description: &str,
    url: &str,
    image: Option<String>,
    body: String,
) -> Response {
    let image = image
        .map(|u| format!("<meta property=\"og:image\" content=\"{}\">", escape(&u)))
        .unwrap_or_default();
    let html = format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{}</title><meta name="description" content="{}"><link rel="canonical" href="{}">
<meta property="og:type" content="article"><meta property="og:title" content="{}"><meta property="og:description" content="{}"><meta property="og:url" content="{}">{}
<meta name="author" content="{}"><style>
:root{{color-scheme:light dark}}body{{font-family:Georgia,serif;line-height:1.65;max-width:740px;margin:auto;padding:40px 22px}}
a{{color:inherit;text-underline-offset:4px}}header{{border-bottom:1px solid #8886;margin-bottom:32px;padding-bottom:24px}}
h1,h2{{line-height:1.2}}h1{{font-size:2.3rem}}small,.byline{{opacity:.7}}.text{{white-space:pre-wrap;overflow-wrap:anywhere}}
img{{max-width:100%;height:auto;border-radius:8px}}.entry{{margin:32px 0}}nav{{display:flex;gap:24px;margin:32px 0}}footer{{margin-top:60px;opacity:.65;font-size:.85rem}}
</style></head><body><header><a href="{}">{}</a><div class="byline">{}</div></header>{}<footer>Publié avec Hupo</footer></body></html>"#,
        escape(title),
        escape(description),
        escape(url),
        escape(title),
        escape(description),
        escape(url),
        image,
        escape(&blog.username),
        escape(&blog_url(blog)),
        escape(&blog.name),
        escape(&blog.username),
        body
    );
    let mut response = Html(html).into_response();
    for (key,value) in [
        (header::CACHE_CONTROL,"no-store"),
        (header::CONTENT_SECURITY_POLICY,"default-src 'none'; img-src 'self' https:; style-src 'unsafe-inline'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'"),
        (header::X_CONTENT_TYPE_OPTIONS,"nosniff"),
        (header::REFERRER_POLICY,"no-referrer"),
    ]{response.headers_mut().insert(key,value.parse().unwrap());}
    response
}
fn entries(blog: &Blog, posts: &PaginatedResponse<BlogPost>, subjects: Option<bool>) -> String {
    posts.items.iter().filter(|p| subjects.map(|s|s==p.source_album_id.is_some()).unwrap_or(true)).map(|p|{
        let subject=if p.source_album_id.is_some(){"Sujet · "}else{""};
        let date=p.last_publication_at.or(p.publishing_date).map(|d|d.format("%Y-%m-%d").to_string()).unwrap_or_default();
        let title=if p.title.trim().is_empty(){"Sans titre"}else{&p.title};
        format!("<section class=\"entry\"><small>{subject}{date}</small><h2><a href=\"{}\">{}</a></h2><p>{}</p></section>",
            escape(&post_url(blog,p)),escape(title),escape(&p.subtitle))
    }).collect()
}
fn pagination(url: &str, posts: &PaginatedResponse<BlogPost>) -> String {
    let mut html = String::from("<nav aria-label=\"Pagination\">");
    if posts.offset > 0 {
        html += &format!(
            "<a href=\"{}?offset={}&amp;limit={}\">← Précédent</a>",
            escape(url),
            (posts.offset - posts.limit).max(0),
            posts.limit
        );
    }
    if posts.offset + posts.limit < posts.total {
        html += &format!(
            "<a href=\"{}?offset={}&amp;limit={}\">Suivant →</a>",
            escape(url),
            posts.offset + posts.limit,
            posts.limit
        );
    }
    html + "</nav>"
}
pub fn home(blog: &Blog, posts: &PaginatedResponse<BlogPost>) -> Response {
    let image = blog.image_asset_id.map(|id| image_url(blog, None, id));
    let cover = image
        .as_ref()
        .map(|u| format!("<img src=\"{}\" alt=\"\">", escape(u)))
        .unwrap_or_default();
    let subjects = entries(blog, posts, Some(true));
    let subjects = if subjects.is_empty() {
        String::new()
    } else {
        format!("<h2>Sujets</h2>{subjects}")
    };
    let body = format!(
        "{cover}<h1>{}</h1><p class=\"text\">{}</p>{subjects}<h2>Articles</h2>{}{}",
        escape(&blog.name),
        escape(&blog.description),
        entries(blog, posts, Some(false)),
        pagination(&blog_url(blog), posts)
    );
    page(
        blog,
        &blog.name,
        &blog.description,
        &blog_url(blog),
        image,
        body,
    )
}
pub fn article(
    blog: &Blog,
    post: &BlogPost,
    items: Option<&PaginatedResponse<BlogPost>>,
) -> Response {
    let image = post
        .image_asset_id
        .map(|id| image_url(blog, Some(post), id));
    let cover = image
        .as_ref()
        .map(|u| format!("<img src=\"{}\" alt=\"\">", escape(u)))
        .unwrap_or_default();
    let date = post
        .last_publication_at
        .or(post.publishing_date)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let mut body=format!("<article><h1>{}</h1><p class=\"byline\">{} · {date}</p><p>{}</p>{cover}<div class=\"text\">{}</div></article>",
        escape(&post.title),escape(&blog.username),escape(&post.subtitle),escape(&post.content));
    if let Some(items) = items {
        body += &format!(
            "<h2>Articles</h2>{}{}",
            entries(blog, items, None),
            pagination(&post_url(blog, post), items)
        );
    }
    page(
        blog,
        &format!("{} — {}", post.title, blog.name),
        &post.subtitle,
        &post_url(blog, post),
        image,
        body,
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn escapes_text_and_attributes() {
        assert_eq!(
            super::escape("<script>\"&'</script>"),
            "&lt;script&gt;&quot;&amp;&#39;&lt;/script&gt;"
        );
    }
}
