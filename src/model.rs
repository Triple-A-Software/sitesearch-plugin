use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ---------------------------------------------------------------------------
// CMS-side rows (read from CMS_DATABASE_URL)
// ---------------------------------------------------------------------------

/// A public page as stored by the Neleto core in the `page` table, joined with
/// the concatenated text of its elements. `body` is the raw HTML content pulled
/// from the CMS `translation` table (keyed `<element-id>:content`); the reindex
/// strips it to plain text before indexing.
#[derive(Debug, Clone, FromRow)]
pub struct CmsPage {
    pub id: i32,
    pub route: String,
    pub title: String,
    pub description: Option<String>,
    pub body: Option<String>,
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

/// A single autocomplete suggestion for the search box.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Suggestion {
    pub url: String,
    pub title: String,
}

/// Query-time search behaviour, editable in the admin panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchSettings {
    /// Rank multiplier for title matches (ts_rank weight for label A).
    pub title_weight: f32,
    /// Rank multiplier for body matches (ts_rank weight for label B).
    pub body_weight: f32,
    /// Groups of equivalent terms; any query word in a group ORs in the rest.
    pub synonyms: Vec<Vec<String>>,
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self {
            title_weight: 1.0,
            body_weight: 0.4,
            synonyms: Vec::new(),
        }
    }
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
