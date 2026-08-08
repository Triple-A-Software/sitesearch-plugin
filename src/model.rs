use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

// ---------------------------------------------------------------------------
// CMS-side rows (read from CMS_DATABASE_URL)
// ---------------------------------------------------------------------------

/// A public page as stored by the Neleto core in the `page` table.
#[derive(Debug, Clone, FromRow)]
pub struct CmsPage {
    pub id: i32,
    pub route: String,
    pub title: String,
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// Extraction result (rewriter)
// ---------------------------------------------------------------------------

/// Content pulled out of a rendered page by the rewriter, ready to be indexed.
#[derive(Debug, Clone, Default)]
pub struct PageContent {
    /// Absolute-or-relative URL the page lives at, derived from canonical /
    /// og:url. Rewriter bodies carry no URL of their own, so a page without
    /// either is skipped (the reindex covers it from the CMS instead).
    pub url: Option<String>,
    pub lang: Option<String>,
    pub title: Option<String>,
    pub body: String,
}

// ---------------------------------------------------------------------------
// Search results
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct SearchResult {
    pub url: String,
    pub r#type: String,
    pub title: String,
    /// `ts_headline` snippet with `<mark>…</mark>` around matched terms.
    pub snippet: String,
    pub rank: f32,
}

// ---------------------------------------------------------------------------
// Admin / dashboard read models
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
pub struct IndexStats {
    /// Total indexed URLs.
    pub indexed: i64,
    /// Rows seeded from the CMS `page` table by a reindex.
    pub indexed_content: i64,
    /// Rows enriched with full page text by the rewriter.
    pub indexed_rendered: i64,
    /// Total searches ever logged.
    pub searches: i64,
    /// Searches that returned no results.
    pub no_result_searches: i64,
    pub top_queries: Vec<QueryCount>,
    pub no_result_queries: Vec<QueryCount>,
    pub last_indexed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct QueryCount {
    pub q: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ReindexResult {
    pub indexed: usize,
    pub total: i64,
}
