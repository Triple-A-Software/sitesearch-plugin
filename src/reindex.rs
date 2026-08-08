//! The content reindex: read every public page from the CMS database and seed
//! the index with its title + description. Instant and complete for pages;
//! full page text is layered on top by the rewriter as pages are viewed.

use sqlx::PgPool;

use crate::{database, utils::AppResult};

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
        let body = p.description.as_deref().unwrap_or("");
        database::upsert_db_page(db, &p.route, Some(p.id), &p.title, body).await?;
        keep.push(p.route.clone());
    }
    database::prune_db_pages(db, &keep).await?;
    Ok(keep.len())
}
