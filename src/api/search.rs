//! The public `/search` results page.
//!
//! For a page with `allow_select_layout`, Neleto renders the visitor's selected
//! layout down to a `<slot></slot>` placeholder and proxies the request to us as
//! a POST whose body is that rendered layout HTML (the original `?q=` query is
//! preserved on the URL). We run the search and splice our results into the
//! slot. When no layout is selected the body is empty and we return a minimal
//! standalone page instead, so the route always works.

use axum::{
    Json,
    extract::{Query, State},
    response::Html,
};
use serde::Deserialize;

use crate::{
    AppState, database,
    model::{SearchResult, Suggestion},
    utils::{AppResult, escape_html},
};

const MAX_QUERY_LEN: usize = 200;
const RESULT_LIMIT: i64 = 20;

#[derive(Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    pub q: Option<String>,
}

/// GET/POST `/search?q=…`. `layout` is the request body: the rendered layout for
/// a POST from Neleto, or empty for a bare GET.
pub async fn page(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
    layout: String,
) -> AppResult<Html<String>> {
    let raw = query.q.unwrap_or_default();
    let q: String = raw.trim().chars().take(MAX_QUERY_LEN).collect();

    let results = if q.is_empty() {
        Vec::new()
    } else {
        database::search(&state.db, &q, RESULT_LIMIT).await?
    };

    if !q.is_empty() {
        // Log the query for the dashboard insights. A logging failure must never
        // break the results page, so we swallow the error.
        if let Err(e) = database::log_query(&state.db, &q, results.len()).await {
            tracing::warn!("failed to log search query: {e}");
        }
    }

    let content = render_content(&q, &results);
    Ok(Html(inject(&layout, &content)))
}

#[derive(Deserialize)]
pub struct SuggestQuery {
    #[serde(default)]
    pub q: Option<String>,
}

/// GET `/search/suggest?q=…` — JSON autocomplete for the search box. Registered
/// as a public `pages` route (no layout) so visitors can reach it; `api` routes
/// would be role-gated.
pub async fn suggest(
    State(state): State<AppState>,
    Query(query): Query<SuggestQuery>,
) -> AppResult<Json<Vec<Suggestion>>> {
    let q: String = query
        .q
        .unwrap_or_default()
        .trim()
        .chars()
        .take(MAX_QUERY_LEN)
        .collect();
    if q.chars().count() < 2 {
        return Ok(Json(Vec::new()));
    }
    Ok(Json(database::suggest(&state.db, &q, 8).await?))
}

/// Build the search UI fragment (refine form + result list) that goes into the
/// layout slot.
fn render_content(q: &str, results: &[SearchResult]) -> String {
    let mut out = String::new();
    out.push_str(STYLE);
    out.push_str(r#"<section class="ss"><form class="ss-form" role="search" action="/search" method="get">"#);
    out.push_str(&format!(
        r#"<input class="ss-input" type="search" name="q" value="{}" placeholder="Suchen…" aria-label="Suchen" autofocus><button class="ss-btn" type="submit">Suchen</button></form>"#,
        escape_html(q)
    ));

    if q.is_empty() {
        out.push_str(r#"<p class="ss-hint">Geben Sie einen Suchbegriff ein.</p>"#);
    } else if results.is_empty() {
        out.push_str(&format!(
            r#"<p class="ss-hint">Keine Ergebnisse für „{}“.</p>"#,
            escape_html(q)
        ));
    } else {
        let noun = if results.len() == 1 { "Ergebnis" } else { "Ergebnisse" };
        out.push_str(&format!(
            r#"<p class="ss-count">{} {} für „{}“</p><ul class="ss-list">"#,
            results.len(),
            noun,
            escape_html(q)
        ));
        for r in results {
            out.push_str(&format!(
                r#"<li class="ss-result"><a class="ss-title" href="{url}"><h3>{title}</h3></a><p class="ss-snippet">{snippet}</p><span class="ss-url">{url}</span></li>"#,
                url = escape_html(&r.url),
                title = escape_html(&r.title),
                snippet = highlight(&r.snippet),
            ));
        }
        out.push_str("</ul>");
    }
    out.push_str("</section>");
    out
}

/// Escape the `ts_headline` snippet, then turn our `[[hl]]…[[/hl]]` sentinels
/// back into `<mark>` tags. Escaping first means any `<`/`>` in the page text
/// can't inject markup; only our own sentinels become HTML.
fn highlight(snippet: &str) -> String {
    escape_html(snippet)
        .replace("[[hl]]", "<mark>")
        .replace("[[/hl]]", "</mark>")
}

/// Splice `content` into the rendered layout. Prefers the `<slot></slot>`
/// placeholder; falls back to just-before-`</body>`; and when there is no layout
/// at all (bare GET) returns a standalone document.
fn inject(layout: &str, content: &str) -> String {
    if layout.trim().is_empty() {
        return standalone(content);
    }
    if let Some(idx) = layout.find("<slot></slot>") {
        let mut s = String::with_capacity(layout.len() + content.len());
        s.push_str(&layout[..idx]);
        s.push_str(content);
        s.push_str(&layout[idx + "<slot></slot>".len()..]);
        return s;
    }
    if let Some(idx) = layout.find("</body>") {
        let mut s = String::with_capacity(layout.len() + content.len());
        s.push_str(&layout[..idx]);
        s.push_str(content);
        s.push_str(&layout[idx..]);
        return s;
    }
    format!("{layout}{content}")
}

fn standalone(content: &str) -> String {
    format!(
        r#"<!doctype html><html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Suche</title></head><body>{content}</body></html>"#
    )
}

const STYLE: &str = r#"<style>
.ss { max-width: 720px; margin: 0 auto; padding: 24px 16px; font-family: system-ui, -apple-system, sans-serif; }
.ss-form { display: flex; gap: 8px; margin-bottom: 20px; }
.ss-input { flex: 1; padding: 10px 12px; border: 1px solid #d1d5db; border-radius: 8px; font: inherit; }
.ss-btn { padding: 10px 16px; border: 0; border-radius: 8px; background: #111827; color: #fff; font: inherit; font-weight: 600; cursor: pointer; }
.ss-hint, .ss-count { color: #6b7280; font-size: 14px; margin: 0 0 16px; }
.ss-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 20px; }
.ss-result { display: flex; flex-direction: column; gap: 2px; }
.ss-title { text-decoration: none; color: inherit; }
.ss-title h3 { margin: 0; font-size: 18px; color: #1d4ed8; }
.ss-snippet { margin: 0; color: #374151; font-size: 14px; line-height: 1.5; }
.ss-snippet mark { background: #fef08a; color: inherit; padding: 0 1px; }
.ss-url { color: #059669; font-size: 12px; }
</style>"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn result(url: &str, title: &str, snippet: &str) -> SearchResult {
        SearchResult {
            url: url.into(),
            r#type: "page".into(),
            title: title.into(),
            snippet: snippet.into(),
            rank: 0.1,
        }
    }

    #[test]
    fn injects_into_slot() {
        let out = inject("<html><body><header>nav</header><slot></slot></body></html>", "RESULTS");
        assert!(out.contains("RESULTS"));
        assert!(!out.contains("<slot>"));
        assert!(out.starts_with("<html>"));
    }

    #[test]
    fn falls_back_to_standalone_when_no_layout() {
        let out = inject("   ", "RESULTS");
        assert!(out.contains("<!doctype html>"));
        assert!(out.contains("RESULTS"));
    }

    #[test]
    fn highlight_escapes_then_marks() {
        // page text with an angle bracket must be escaped; sentinels become mark
        let s = highlight("a [[hl]]<b>[[/hl]] c");
        assert_eq!(s, "a <mark>&lt;b&gt;</mark> c");
    }

    #[test]
    fn renders_results_and_escapes_query() {
        let content = render_content("<script>", &[result("/a", "A", "hi")]);
        assert!(content.contains("&lt;script&gt;"));
        assert!(!content.contains("<script>"));
        assert!(content.contains(r#"href="/a""#));
    }
}
