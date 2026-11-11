use super::{
    model::{Blog, Publication},
    urls,
};
use crate::pagination::PaginatedResponse;
use axum::{
    http::header,
    response::{Html, IntoResponse, Response},
};
use pulldown_cmark::{html, Event, Options, Parser, TagEnd};

fn markdown_parser(content: &str) -> Parser<'_> {
    Parser::new_ext(
        content,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
    )
}

fn render_markdown(content: &str) -> String {
    let events = markdown_parser(content).map(|event| match event {
        // Authors may use Markdown, not inject arbitrary HTML into a public page.
        Event::Html(text) | Event::InlineHtml(text) => Event::Text(text),
        _ => event,
    });
    let mut rendered = String::new();
    html::push_html(&mut rendered, events);
    // Sanitize the generated fragment as well, including decoded/obfuscated URLs.
    ammonia::Builder::default()
        .url_schemes(["http", "https", "mailto"].into_iter().collect())
        .clean(&rendered)
        .to_string()
}

fn markdown_plain_text(content: &str) -> String {
    let mut text = String::new();
    for event in markdown_parser(content) {
        match event {
            Event::Text(value) | Event::Code(value) => text.push_str(&value),
            Event::SoftBreak | Event::HardBreak | Event::Rule => text.push(' '),
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::Item
                | TagEnd::BlockQuote(_)
                | TagEnd::CodeBlock
                | TagEnd::TableCell,
            ) => text.push(' '),
            _ => (),
        }
    }
    text
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn base() -> String {
    urls::base()
}
fn blog_url(blog: &Blog) -> String {
    format!("{}/{}", base(), blog.slug)
}
fn post_url(blog: &Blog, post: &Publication) -> String {
    urls::publication(&blog.slug, post.id)
}
fn image_url(blog: &Blog, post: Option<&Publication>, asset: uuid::Uuid) -> String {
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
    let logo_url = escape(&format!("{}/_assets/hupo.svg", base()));
    let card = if image.is_some() {
        "summary_large_image"
    } else {
        "summary"
    };
    let logo_metadata = if image.is_none() {
        "<meta property=\"og:image:type\" content=\"image/png\"><meta property=\"og:image:width\" content=\"1024\"><meta property=\"og:image:height\" content=\"1024\"><meta property=\"og:image:alt\" content=\"Logo Hupo\"><meta name=\"twitter:image:alt\" content=\"Logo Hupo\">"
    } else {
        ""
    };
    let preview_url = image.unwrap_or_else(|| format!("{}/_assets/hupo.png", base()));
    let image = format!("<meta property=\"og:image\" content=\"{}\"><meta name=\"twitter:image\" content=\"{}\">{logo_metadata}", escape(&preview_url), escape(&preview_url));
    let og_type = if url == blog_url(blog) {
        "website"
    } else {
        "article"
    };
    let site_name = escape(&blog.name);
    let twitter_title = escape(title);
    let twitter_description = escape(description);
    let html = format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{}</title><meta name="description" content="{}"><link rel="canonical" href="{}">
<link rel="icon" type="image/svg+xml" href="{logo_url}">
<meta property="og:type" content="{og_type}"><meta property="og:site_name" content="{site_name}"><meta property="og:title" content="{}"><meta property="og:description" content="{}"><meta property="og:url" content="{}">{}
<meta name="twitter:card" content="{card}"><meta name="twitter:title" content="{twitter_title}"><meta name="twitter:description" content="{twitter_description}">
<meta name="author" content="{}"><style>
:root{{color-scheme:light dark}}body{{font-family:Georgia,serif;line-height:1.65;max-width:740px;margin:auto;padding:40px 22px}}
a{{color:inherit;text-underline-offset:4px}}header{{border-bottom:1px solid #8886;margin-bottom:32px;padding-bottom:24px}}
.blog-heading{{display:flex;align-items:center;justify-content:space-between;gap:12px;flex-wrap:wrap}}.hupo-home{{font: .85rem system-ui,sans-serif;opacity:.8;white-space:nowrap}}
h1,h2{{line-height:1.2}}h1{{font-size:2.3rem}}small,.byline{{opacity:.7}}.text{{white-space:pre-wrap;overflow-wrap:anywhere}}
.markdown{{overflow-wrap:anywhere}}.markdown p,.markdown ul,.markdown ol,.markdown blockquote,.markdown pre{{margin:1em 0}}
.markdown h1,.markdown h2,.markdown h3,.markdown h4,.markdown h5,.markdown h6{{line-height:1.25;margin:1.4em 0 .6em}}
.markdown blockquote{{margin-left:0;padding-left:1em;border-left:3px solid #8886}}
.markdown pre{{padding:1em;overflow-x:auto;border:1px solid #8886;border-radius:6px;white-space:pre;overflow-wrap:normal}}
.markdown code{{font-size:.9em}}.markdown :not(pre)>code{{padding:.1em .3em;background:#8882;border-radius:3px}}
.markdown table{{display:block;max-width:100%;overflow-x:auto;border-collapse:collapse}}.markdown th,.markdown td{{border:1px solid #8886;padding:.4em .7em}}
img{{max-width:100%;height:auto;border-radius:8px}}.entry{{margin:32px 0}}nav{{display:flex;gap:24px;margin:32px 0}}footer{{margin-top:60px;opacity:.65;font-size:.85rem}}
footer a{{display:inline-flex;align-items:center;gap:8px;text-decoration:none}}footer img{{width:32px;height:32px;flex-shrink:0}}.excerpt{{opacity:.85;margin-top:8px}}
.sharing{{margin:24px 0;font:inherit}}.sharing button,.sharing summary{{cursor:pointer}}.sharing button{{font:inherit;color:inherit;background:transparent;border:1px solid #8886;border-radius:6px;padding:.4em .8em}}.sharing input{{box-sizing:border-box;width:100%;font:inherit;color:inherit;background:transparent;border:1px solid #8886;border-radius:6px;padding:.5em;margin:.5em 0}}.sharing [role=status]{{display:block;font-size:.85rem;margin-top:.5em}}.sharing label{{display:block;margin-top:.5em}}.sharing a{{font-size:.85rem}}
</style></head><body><header><div class="blog-heading"><a href="{}">{}</a><a class="hupo-home" href="https://hupo.fr">Découvrir Hupo ↗</a></div><div class="byline">{}</div></header>{}<footer><a href="https://hupo.fr">Publié avec <img src="{logo_url}" width="32" height="32" alt="">Hupo</a></footer></body></html>"#,
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
        (header::CONTENT_SECURITY_POLICY,"default-src 'none'; img-src 'self' https:; style-src 'unsafe-inline'; script-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'"),
        (header::X_CONTENT_TYPE_OPTIONS,"nosniff"),
        (header::REFERRER_POLICY,"no-referrer"),
    ]{response.headers_mut().insert(key,value.parse().unwrap());}
    response
}
fn excerpt(content: &str) -> String {
    const LIMIT: usize = 240;
    let text = markdown_plain_text(content)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut chars = text.chars();
    let prefix: String = chars.by_ref().take(LIMIT).collect();
    if chars.next().is_none() {
        return text;
    }
    let end = if prefix.ends_with(' ') {
        prefix.trim_end()
    } else {
        prefix
            .rsplit_once(' ')
            .map(|(words, _)| words)
            .unwrap_or(&prefix)
    };
    format!("{end}…")
}
fn entries(blog: &Blog, posts: &PaginatedResponse<Publication>, subjects: Option<bool>) -> String {
    posts.items.iter().filter(|p| subjects.map(|s|s==p.source_album_id.is_some()).unwrap_or(true)).map(|p|{
        let subject=if p.source_album_id.is_some(){"Sujet · "}else{""};
        let date=p.last_distribution_at.or(p.publishing_date).map(|d|d.format("%Y-%m-%d").to_string()).unwrap_or_default();
        let title=if p.title.trim().is_empty(){"Sans titre"}else{&p.title};
        let subtitle=if p.subtitle.trim().is_empty(){String::new()}else{format!("<p>{}</p>",escape(&p.subtitle))};
        let preview=if p.source_trace_id.is_some(){excerpt(&p.content)}else{String::new()};
        let preview=if preview.is_empty(){String::new()}else{format!("<p class=\"excerpt\">{}</p>",escape(&preview))};
        format!("<section class=\"entry\"><small>{subject}{date}</small><h2><a href=\"{}\">{}</a></h2>{subtitle}{preview}</section>",
            escape(&post_url(blog,p)),escape(title))
    }).collect()
}
fn pagination(url: &str, posts: &PaginatedResponse<Publication>) -> String {
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
pub fn home(blog: &Blog, posts: &PaginatedResponse<Publication>) -> Response {
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
    post: &Publication,
    items: Option<&PaginatedResponse<Publication>>,
) -> Response {
    let image = post
        .image_asset_id
        .map(|id| image_url(blog, Some(post), id));
    let preview_image = image.clone();
    let cover = image
        .as_ref()
        .map(|u| format!("<img src=\"{}\" alt=\"\">", escape(u)))
        .unwrap_or_default();
    let date = post
        .last_distribution_at
        .or(post.publishing_date)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let short_url = escape(&urls::short(&post.short_code));
    let script_url = escape(&format!("{}/_assets/share.js", base()));
    let sharing = format!(
        r#"<div class="sharing"><button id="share-article" type="button" hidden>Partager</button><details id="share-fallback"><summary>Partager</summary><label for="share-url">Lien court de cet article</label><input id="share-url" type="text" readonly value="{short_url}"><button id="copy-share-url" type="button" hidden>Copier le lien</button> <a href="{short_url}">Ouvrir le lien court</a></details><span id="share-status" role="status" aria-live="polite"></span></div><script src="{script_url}" defer></script>"#
    );
    let mut body=format!("<article><h1>{}</h1><p class=\"byline\">{} · {date}</p>{sharing}<p>{}</p>{cover}<div class=\"markdown\">{}</div></article>",
        escape(&post.title),escape(&blog.username),escape(&post.subtitle),render_markdown(&post.content));
    if let Some(items) = items {
        body += &format!(
            "<h2>Articles</h2>{}{}",
            entries(blog, items, None),
            pagination(&post_url(blog, post), items)
        );
    }
    let description = if post.subtitle.trim().is_empty() {
        excerpt(&post.content)
    } else {
        post.subtitle.trim().to_owned()
    };
    page(
        blog,
        &format!("{} — {}", post.title, blog.name),
        &description,
        &post_url(blog, post),
        preview_image,
        body,
    )
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn article_previews_have_open_graph_twitter_and_excerpt_fallbacks() {
        let now = chrono::Utc::now().naive_utc();
        let mut blog = super::Blog {
            id: uuid::Uuid::new_v4(),
            user_id: uuid::Uuid::new_v4(),
            slug: "preview-blog".into(),
            name: "A blog".into(),
            username: "A pseudonym".into(),
            description: "Introduction".into(),
            image_asset_id: Some(uuid::Uuid::new_v4()),
            enabled: true,
            created_at: now,
            updated_at: now,
        };
        let mut post = super::Publication {
            id: uuid::Uuid::new_v4(),
            short_code: "k7Mx2Qa9".into(),
            publication_channel_id: blog.id,
            source_trace_id: Some(uuid::Uuid::new_v4()),
            source_album_id: None,
            status: "PUBLISHED".into(),
            publishing_date: Some(now),
            last_distribution_event_id: None,
            last_distribution_at: None,
            ordering_date: None,
            title: "A \"quoted\" title".into(),
            subtitle: "".into(),
            content: "## Opening\n\nA **clear** thought.".into(),
            image_asset_id: None,
            finalized_at: Some(now),
        };
        let response = super::article(&blog, &post, None);
        let csp = response.headers()[axum::http::header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap();
        assert!(csp.contains("script-src 'self';"));
        assert!(!csp.contains("script-src 'unsafe-inline'"));
        let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .unwrap();
        let html = std::str::from_utf8(&bytes).unwrap();
        let document = scraper::Html::parse_document(html);
        let share_input = scraper::Selector::parse("#share-url[readonly]").unwrap();
        assert_eq!(
            document
                .select(&share_input)
                .next()
                .unwrap()
                .value()
                .attr("value"),
            Some(super::urls::short(&post.short_code).as_str())
        );
        let script = scraper::Selector::parse("script[src][defer]").unwrap();
        assert_eq!(
            document.select(&script).next().unwrap().value().attr("src"),
            Some(format!("{}/_assets/share.js", super::base()).as_str())
        );
        let meta = |attribute: &str, name: &str| {
            let selector =
                scraper::Selector::parse(&format!("meta[{attribute}='{name}']")).unwrap();
            document
                .select(&selector)
                .next()
                .and_then(|node| node.value().attr("content"))
                .map(str::to_owned)
        };
        assert_eq!(
            meta("property", "og:title").as_deref(),
            Some("A \"quoted\" title — A blog")
        );
        assert_eq!(
            meta("property", "og:description").as_deref(),
            Some("Opening A clear thought.")
        );
        assert_eq!(
            meta("property", "og:url"),
            Some(super::post_url(&blog, &post))
        );
        assert_eq!(meta("name", "twitter:card").as_deref(), Some("summary"));
        assert_eq!(meta("name", "twitter:title"), meta("property", "og:title"));
        assert_eq!(
            meta("name", "twitter:description"),
            meta("property", "og:description")
        );
        assert_eq!(meta("name", "twitter:image"), meta("property", "og:image"));
        assert_eq!(
            meta("name", "twitter:image"),
            Some(format!("{}/_assets/hupo.png", super::base()))
        );
        assert_eq!(
            meta("property", "og:image:type").as_deref(),
            Some("image/png")
        );
        assert_eq!(meta("property", "og:image:width").as_deref(), Some("1024"));
        // A preview fallback is not inserted as a cover in the article body.
        let cover = scraper::Selector::parse("article > img").unwrap();
        assert!(document.select(&cover).next().is_none());
        let selector = scraper::Selector::parse("link[rel='canonical']").unwrap();
        assert_eq!(
            document
                .select(&selector)
                .next()
                .unwrap()
                .value()
                .attr("href"),
            Some(super::post_url(&blog, &post).as_str())
        );
        blog.image_asset_id = None;
        let response = super::article(&blog, &post, None);
        let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .unwrap();
        let html = std::str::from_utf8(&bytes).unwrap();
        assert!(html.contains("name=\"twitter:card\" content=\"summary\""));
        assert!(html.contains(&format!(
            "name=\"twitter:image\" content=\"{}/_assets/hupo.png\"",
            super::base()
        )));

        post.image_asset_id = Some(uuid::Uuid::new_v4());
        let response = super::article(&blog, &post, None);
        let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .unwrap();
        let html = std::str::from_utf8(&bytes).unwrap();
        assert!(html.contains("name=\"twitter:card\" content=\"summary_large_image\""));
        assert!(html.contains(&format!(
            "name=\"twitter:image\" content=\"{}\"",
            super::image_url(&blog, Some(&post), post.image_asset_id.unwrap())
        )));
        assert!(!html.contains("/_assets/hupo.png"));
    }

    #[test]
    fn markdown_renders_readable_blocks_and_inline_formatting() {
        let html = super::render_markdown(concat!(
            "## A heading\n\nA **bold** and *italic* paragraph with `code`.\n\n",
            "- First\n- Second\n\n> A quote\n\n```rust\nlet x = 1;\n```\n\n",
            "| A | B |\n| - | - |\n| one | two |\n\n~~Old~~\n\n",
            "[A link](https://example.com/?a=1&b=2) and [relative](/notes).\n\n",
            "![A photo](https://example.com/image.jpg)\n\nNew\nline\n\nNew paragraph."
        ));
        for expected in [
            "<h2>A heading</h2>",
            "<strong>bold</strong>",
            "<em>italic</em>",
            "<code>code</code>",
            "<ul>",
            "<blockquote>",
            "<pre><code",
            "<table>",
            "<del>Old</del>",
            "href=\"https://example.com/?a=1&amp;b=2\"",
            "href=\"/notes\"",
            "src=\"https://example.com/image.jpg\"",
            "<p>New\nline</p>",
            "<p>New paragraph.</p>",
        ] {
            assert!(html.contains(expected), "Missing {expected} in {html}");
        }
        assert_eq!(
            super::excerpt("## Heading\n\nAn im**port**ant [link](https://example.com)."),
            "Heading An important link."
        );
    }

    #[test]
    fn markdown_disables_raw_html_and_unsafe_urls() {
        let html =
            super::render_markdown("<img src=x onerror=alert(1)>\n\n<script>alert(1)</script>");
        assert!(!html.contains("<img"));
        assert!(!html.contains("<script"));
        assert!(html.contains("&lt;script&gt;"));
        for url in [
            "javascript:alert(1)",
            "JaVaScRiPt:alert(1)",
            "java&#x73;cript:alert(1)",
            "java&#9;script:alert(1)",
            "data:text/html,hello",
            "vbscript:msgbox(1)",
            "file:///etc/passwd",
        ] {
            let html = super::render_markdown(&format!("[click]({url}) ![image]({url})"));
            let document = scraper::Html::parse_fragment(&html);
            let selector = scraper::Selector::parse("a[href], img[src]").unwrap();
            let base = url::Url::parse("https://example.com/").unwrap();
            // Malformed destinations may be safely percent-encoded as relative
            // paths; check their effective scheme, not just attribute presence.
            for node in document.select(&selector) {
                let value = node
                    .value()
                    .attr("href")
                    .or_else(|| node.value().attr("src"))
                    .unwrap();
                let target = base.join(value).unwrap();
                assert!(
                    matches!(target.scheme(), "http" | "https" | "mailto"),
                    "Unsafe URL survived: {html}"
                );
            }
        }
    }

    #[test]
    fn excerpts_are_short_unicode_safe_and_normalize_whitespace() {
        assert_eq!(super::excerpt(" Bonjour\n\n  à tous. "), "Bonjour à tous.");
        assert_eq!(super::excerpt(" \n\t "), "");
        let long = super::excerpt(&"écriture ".repeat(50));
        assert!(long.chars().count() <= 241);
        assert!(long.ends_with("écriture…"));
        assert_eq!(
            super::excerpt(&"é".repeat(250)),
            format!("{}…", "é".repeat(240))
        );
    }

    #[test]
    fn escapes_text_and_attributes() {
        assert_eq!(
            super::escape("<script>\"&'</script>"),
            "&lt;script&gt;&quot;&amp;&#39;&lt;/script&gt;"
        );
    }

    #[tokio::test]
    async fn shared_page_includes_hupo_logo_and_favicon() {
        let now = chrono::Utc::now().naive_utc();
        let blog = super::Blog {
            id: uuid::Uuid::new_v4(),
            user_id: uuid::Uuid::new_v4(),
            slug: "test-blog".into(),
            name: "Test blog".into(),
            username: "Pseudonym".into(),
            description: String::new(),
            image_asset_id: None,
            enabled: true,
            created_at: now,
            updated_at: now,
        };
        let response = super::page(&blog, "Test", "", "/test-blog", None, String::new());
        let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .unwrap();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(html.contains("rel=\"icon\" type=\"image/svg+xml\""));
        assert_eq!(html.matches("/_assets/hupo.svg").count(), 2);
        assert!(html.contains(&format!(
            "property=\"og:image\" content=\"{}/_assets/hupo.png\"",
            super::base()
        )));
        assert!(html.contains("<footer><a href=\"https://hupo.fr\">"));
        let document = scraper::Html::parse_document(html);
        let selector = scraper::Selector::parse("header a.hupo-home").unwrap();
        let link = document.select(&selector).next().unwrap();
        assert_eq!(link.value().attr("href"), Some("https://hupo.fr"));
        assert_eq!(link.text().collect::<String>(), "Découvrir Hupo ↗");
        assert!(html.contains("width=\"32\" height=\"32\""));
    }
}
