//! The content reindex: read every public page from the CMS database and seed
//! the index with its title + description. Instant and complete for pages;
//! full page text is layered on top by the rewriter as pages are viewed.

use sqlx::PgPool;

use crate::{database, indexer, utils::AppResult};

/// Reindex all public pages, then prune content rows for pages that no longer
/// exist. Returns the number of pages indexed.
pub async fn run_reindex(db: &PgPool, cms_db: &PgPool) -> AppResult<usize> {
    let pages = database::fetch_cms_pages(cms_db).await?;
    let mut keep: Vec<String> = Vec::with_capacity(pages.len());
    for p in &pages {
        // Skip template routes with parameters (e.g. /blog/:slug, /shop/*): they
        // are not real URLs. Their concrete instances get indexed by the
        // rewriter when they're actually rendered.
        if p.route.contains(':') || p.route.contains('*') {
            continue;
        }
        // Body = the page's description plus the plain text of its elements, so
        // full-text search matches page content and not just title/description.
        let description = p.description.as_deref().unwrap_or("");
        let content = p.body.as_deref().map(indexer::strip_html).unwrap_or_default();
        let body = format!("{description} {content}");
        database::upsert_db_page(db, &p.route, Some(p.id), &p.title, body.trim()).await?;
        keep.push(p.route.clone());
    }
    database::prune_db_pages(db, &keep).await?;
    Ok(keep.len())
}
