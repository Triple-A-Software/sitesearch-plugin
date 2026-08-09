//! Pure extraction of indexable content from a rendered HTML page. The rewriter
//! hook hands us the full document of every page render; we pull out the URL
//! (from canonical / og:url — the body itself carries none), language, title,
//! and the visible text of the main content blocks.

use scraper::{Html, Selector};

use crate::model::PageContent;

/// Text-bearing content blocks. Deliberately excludes `<script>` / `<style>`
/// (whose text would otherwise pollute the index) and chrome-only wrappers.
const CONTENT_SELECTOR: &str =
    "h1,h2,h3,h4,h5,h6,p,li,dd,dt,td,th,blockquote,figcaption,summary,caption";

const MAX_BODY_LEN: usize = 20_000;

pub fn extract(html: &str) -> PageContent {
    let doc = Html::parse_document(html);

    let attr = |css: &str, name: &str| -> Option<String> {
        let selector = Selector::parse(css).ok()?;
        doc.select(&selector)
            .next()
            .and_then(|el| el.value().attr(name))
            .map(str::to_string)
    };

    let url = attr("link[rel=canonical]", "href")
        .or_else(|| attr(r#"meta[property="og:url"]"#, "content"))
        .map(|u| normalize_path(&u))
        .filter(|p| !p.is_empty());

    let lang = attr("html", "lang")
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty());

    let title = Selector::parse("title")
        .ok()
        .and_then(|s| doc.select(&s).next().map(|el| collapse_ws(&el.text().collect::<String>())))
        .filter(|t| !t.is_empty())
        .or_else(|| {
            attr(r#"meta[property="og:title"]"#, "content").map(|t| collapse_ws(&t))
        })
        .filter(|t| !t.is_empty());

    PageContent {
        url,
        lang,
        title,
        body: extract_text(&doc),
    }
}

/// Turn a CMS content fragment (stored as HTML in the `translation` table) into
/// collapsed plain text for indexing. Unlike [`extract`], which parses a whole
/// rendered document and pulls specific content blocks, this handles the bare
/// HTML of a single element's content — text nodes are joined with spaces so
/// adjacent nodes across tags keep a word boundary.
pub fn strip_html(fragment: &str) -> String {
    let doc = Html::parse_fragment(fragment);
    let text = doc.root_element().text().collect::<Vec<_>>().join(" ");
    let mut body = collapse_ws(&text);
    if body.len() > MAX_BODY_LEN {
        let mut end = MAX_BODY_LEN;
        while !body.is_char_boundary(end) {
            end -= 1;
        }
        body.truncate(end);
    }
    body
}

fn extract_text(doc: &Html) -> String {
    let selector = Selector::parse(CONTENT_SELECTOR).expect("static selector is valid");
    let mut parts: Vec<String> = Vec::new();
    for el in doc.select(&selector) {
        let text = collapse_ws(&el.text().collect::<String>());
        if !text.is_empty() {
            parts.push(text);
        }
    }
    let mut body = parts.join(" ");
    if body.len() > MAX_BODY_LEN {
        let mut end = MAX_BODY_LEN;
        while !body.is_char_boundary(end) {
            end -= 1;
        }
        body.truncate(end);
    }
    body
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Reduce a canonical/og:url to a site-relative path so a rendered page keys to
/// the same index row as the CMS `page.route` it was reindexed from. Drops the
/// scheme+host, any query/hash, and a trailing slash (except the root).
fn normalize_path(u: &str) -> String {
    let s = u.trim();
    let s = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .map(|rest| match rest.find('/') {
            Some(i) => &rest[i..],
            None => "/",
        })
        .unwrap_or(s);
    let s = s.split(['?', '#']).next().unwrap_or(s);
    let trimmed = s.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_absolute_and_relative_urls() {
        assert_eq!(normalize_path("https://example.com/about"), "/about");
        assert_eq!(normalize_path("http://example.com/a/b/"), "/a/b");
        assert_eq!(normalize_path("https://example.com/"), "/");
        assert_eq!(normalize_path("/contact?utm=x#top"), "/contact");
        assert_eq!(normalize_path("  /team/  "), "/team");
    }

    #[test]
    fn extracts_url_title_and_visible_text_only() {
        let html = r#"<!doctype html>
        <html lang="de">
        <head>
            <title>  Über   uns </title>
            <link rel="canonical" href="https://example.com/ueber-uns" />
            <style>.x { color: red }</style>
        </head>
        <body>
            <script>var secret = "should not be indexed";</script>
            <main><h1>Unser Team</h1><p>Wir bauen Software.</p></main>
        </body>
        </html>"#;
        let c = extract(html);
        assert_eq!(c.url.as_deref(), Some("/ueber-uns"));
        assert_eq!(c.lang.as_deref(), Some("de"));
        assert_eq!(c.title.as_deref(), Some("Über uns"));
        assert!(c.body.contains("Unser Team"));
        assert!(c.body.contains("Wir bauen Software."));
        assert!(!c.body.contains("secret"));
        assert!(!c.body.contains("color"));
    }

    #[test]
    fn no_url_when_no_canonical_or_og() {
        let c = extract("<html><body><p>hi</p></body></html>");
        assert!(c.url.is_none());
    }

    #[test]
    fn strip_html_keeps_word_boundaries_across_tags() {
        // adjacent block/inline text must not run together into one token
        let s = strip_html("<p>Hallo <strong>Welt</strong></p><p>Lorem ipsum</p>");
        assert_eq!(s, "Hallo Welt Lorem ipsum");
    }

    #[test]
    fn strip_html_handles_bare_text() {
        assert_eq!(strip_html("just text"), "just text");
        assert_eq!(strip_html(""), "");
    }
}
