use axum::extract::State;

use crate::{AppState, database, indexer};

/// Neleto POSTs the fully-rendered HTML of every page here and replaces the page
/// with our 200 response. We return the HTML untouched (search never mutates the
/// page) and index it in a detached task so we never add latency to a page load.
///
/// The rewriter body carries no URL, so we key the index on the canonical /
/// og:url found inside the document. Pages without either are skipped — the
/// content reindex already covers them from the CMS `page` table.
pub async fn rewriter(State(state): State<AppState>, body: String) -> String {
    let db = state.db.clone();
    let html = body.clone();
    tokio::spawn(async move {
        let content = indexer::extract(&html);
        let Some(url) = content.url.clone() else {
            return;
        };
        // Nothing worth indexing (e.g. an error/redirect shell) — leave any
        // existing row untouched.
        let has_title = content.title.as_deref().is_some_and(|t| !t.is_empty());
        if !has_title && content.body.is_empty() {
            return;
        }
        if let Err(e) = database::upsert_rendered(
            &db,
            &url,
            content.lang.as_deref(),
            content.title.as_deref(),
            &content.body,
        )
        .await
        {
            tracing::warn!("rewriter: failed to index {url}: {e}");
        }
    });
    body
}
