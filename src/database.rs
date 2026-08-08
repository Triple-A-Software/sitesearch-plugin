use sqlx::PgPool;

use crate::{
    model::{CmsPage, IndexStats, QueryCount, SearchResult},
    utils::AppResult,
};

// ---------------------------------------------------------------------------
// CMS reads
// ---------------------------------------------------------------------------

/// All public, non-deleted pages from the CMS.
pub async fn fetch_cms_pages(cms_db: &PgPool) -> AppResult<Vec<CmsPage>> {
    Ok(sqlx::query_as(
        r#"select id, route, title, description
           from page
           where status = 'public' and deleted_at is null"#,
    )
    .fetch_all(cms_db)
    .await?)
}

// ---------------------------------------------------------------------------
// Index writes
// ---------------------------------------------------------------------------

/// Seed/refresh a page from the CMS content reindex. Title is always taken as
/// authoritative from the CMS; the body baseline (description) is only written
/// when the row hasn't already been enriched with full text by the rewriter, so
/// a reindex never downgrades a richer rendered row.
pub async fn upsert_db_page(
    db: &PgPool,
    url: &str,
    page_id: Option<i32>,
    title: &str,
    body: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"insert into search_index (url, page_id, type, source, title, body, updated_at)
           values ($1, $2, 'page', 'db', $3, $4, now())
           on conflict (url) do update set
               page_id = coalesce(excluded.page_id, search_index.page_id),
               title = excluded.title,
               body = case when search_index.source = 'rendered'
                           then search_index.body else excluded.body end,
               updated_at = now()"#,
    )
    .bind(url)
    .bind(page_id)
    .bind(title)
    .bind(body)
    .execute(db)
    .await?;
    Ok(())
}

/// Upsert full page text extracted from a rendered page by the rewriter. Marks
/// the row `rendered` so a later content reindex won't clobber the richer body.
pub async fn upsert_rendered(
    db: &PgPool,
    url: &str,
    lang: Option<&str>,
    title: Option<&str>,
    body: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"insert into search_index (url, type, source, lang, title, body, updated_at)
           values ($1, 'page', 'rendered', $2, $3, $4, now())
           on conflict (url) do update set
               source = 'rendered',
               lang = coalesce(excluded.lang, search_index.lang),
               title = coalesce(nullif(excluded.title, ''), search_index.title),
               body = excluded.body,
               updated_at = now()"#,
    )
    .bind(url)
    .bind(lang)
    .bind(title)
    .bind(body)
    .execute(db)
    .await?;
    Ok(())
}

/// Drop content-reindex rows for pages that no longer exist. Only `source = 'db'`
/// rows are pruned; rendered rows (which may be posts/events with no CMS `page`
/// entry) are left alone.
pub async fn prune_db_pages(db: &PgPool, keep: &[String]) -> AppResult<()> {
    sqlx::query(r#"delete from search_index where source = 'db' and not (url = any($1))"#)
        .bind(keep)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn count_indexed(db: &PgPool) -> AppResult<i64> {
    Ok(sqlx::query_scalar::<_, i64>(r#"select count(*) from search_index"#)
        .fetch_one(db)
        .await?)
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

/// Full-text search over the index. Matched terms in the snippet are wrapped in
/// `[[hl]]…[[/hl]]` sentinels (not HTML) so the caller can HTML-escape the whole
/// snippet and then safely turn the sentinels into `<mark>` tags.
pub async fn search(db: &PgPool, q: &str, limit: i64) -> AppResult<Vec<SearchResult>> {
    let rows: Vec<(String, String, String, String, f32)> = sqlx::query_as(
        r#"
        with query as (select websearch_to_tsquery('german', $1) as q)
        select si.url,
               si.type,
               si.title,
               ts_headline(
                   'german',
                   coalesce(nullif(si.body, ''), si.title),
                   query.q,
                   'StartSel=[[hl]], StopSel=[[/hl]], MaxFragments=2, MinWords=6, MaxWords=24'
               ) as snippet,
               ts_rank(si.tsv, query.q) as rank
        from search_index si, query
        where query.q is not null and si.tsv @@ query.q
        order by rank desc, si.updated_at desc
        limit $2
        "#,
    )
    .bind(q)
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(url, r#type, title, snippet, rank)| SearchResult {
            url,
            r#type,
            title,
            snippet,
            rank,
        })
        .collect())
}

pub async fn log_query(db: &PgPool, q: &str, results_count: usize) -> AppResult<()> {
    sqlx::query(r#"insert into search_query_log (q, results_count) values ($1, $2)"#)
        .bind(q)
        .bind(results_count as i32)
        .execute(db)
        .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Admin / dashboard reads
// ---------------------------------------------------------------------------

async fn top_queries(db: &PgPool, only_no_result: bool, limit: i64) -> AppResult<Vec<QueryCount>> {
    let where_clause = if only_no_result { "where results_count = 0" } else { "" };
    Ok(sqlx::query_as(&format!(
        r#"select lower(btrim(q)) as q, count(*)::int8 as count
           from search_query_log
           {where_clause}
           group by lower(btrim(q))
           order by count desc, q asc
           limit $1"#,
    ))
    .bind(limit)
    .fetch_all(db)
    .await?)
}

pub async fn top_queries_public(db: &PgPool, limit: i64) -> AppResult<Vec<QueryCount>> {
    top_queries(db, false, limit).await
}

pub async fn no_result_queries(db: &PgPool, limit: i64) -> AppResult<Vec<QueryCount>> {
    top_queries(db, true, limit).await
}

pub async fn stats(db: &PgPool) -> AppResult<IndexStats> {
    let scalar = |sql: &'static str| async move {
        sqlx::query_scalar::<_, i64>(sql).fetch_one(db).await
    };
    Ok(IndexStats {
        indexed: scalar("select count(*) from search_index").await?,
        indexed_content: scalar("select count(*) from search_index where source = 'db'").await?,
        indexed_rendered: scalar("select count(*) from search_index where source = 'rendered'").await?,
        searches: scalar("select count(*) from search_query_log").await?,
        no_result_searches: scalar("select count(*) from search_query_log where results_count = 0").await?,
        top_queries: top_queries(db, false, 10).await?,
        no_result_queries: top_queries(db, true, 10).await?,
        last_indexed_at: sqlx::query_scalar(r#"select max(updated_at) from search_index"#)
            .fetch_one(db)
            .await?,
    })
}
