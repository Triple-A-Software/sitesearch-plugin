use sqlx::PgPool;

use crate::{
    model::{CmsPage, IndexStats, QueryCount, SearchResult, SearchSettings, Suggestion},
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

fn to_results(rows: Vec<(String, String, String, String, f32)>) -> Vec<SearchResult> {
    rows.into_iter()
        .map(|(url, r#type, title, snippet, rank)| SearchResult {
            url,
            r#type,
            title,
            snippet,
            rank,
        })
        .collect()
}

/// Full-text search over the index. Applies the configured title/body weighting
/// and synonym expansion, and falls back to trigram similarity when the exact
/// query finds nothing (typo tolerance). Matched terms in the snippet are
/// wrapped in `[[hl]]…[[/hl]]` sentinels (not HTML) so the caller can
/// HTML-escape the whole snippet and then safely turn the sentinels into
/// `<mark>` tags.
pub async fn search(db: &PgPool, q: &str, limit: i64) -> AppResult<Vec<SearchResult>> {
    let settings = get_settings(db).await?;
    let expansion = synonym_expansion(q, &settings.synonyms);
    // ts_rank's weight array is {D, C, B, A}; the index labels title 'A', body 'B'.
    let weights = vec![0.1_f32, 0.2, settings.body_weight, settings.title_weight];

    let rows: Vec<(String, String, String, String, f32)> = sqlx::query_as(
        r#"
        with tsq as (
            select (
                websearch_to_tsquery('german', $1)
                || case when $2 = '' then ''::tsquery else to_tsquery('german', $2) end
            ) as q
        )
        select si.url,
               si.type,
               si.title,
               ts_headline(
                   'german',
                   coalesce(nullif(si.body, ''), si.title),
                   tsq.q,
                   'StartSel=[[hl]], StopSel=[[/hl]], MaxFragments=2, MinWords=6, MaxWords=24'
               ) as snippet,
               ts_rank($3, si.tsv, tsq.q) as rank
        from search_index si, tsq
        where si.tsv @@ tsq.q
        order by rank desc, si.updated_at desc
        limit $4
        "#,
    )
    .bind(q)
    .bind(&expansion)
    .bind(&weights)
    .bind(limit)
    .fetch_all(db)
    .await?;

    let results = to_results(rows);
    if results.is_empty() && !q.trim().is_empty() {
        return fuzzy_search(db, q.trim(), limit).await;
    }
    Ok(results)
}

/// Typo-tolerant fallback: trigram similarity on the title (pg_trgm) so
/// "gebaeude" still finds "Gebäude". No `ts_headline` here — the snippet is a
/// plain body/title prefix.
async fn fuzzy_search(db: &PgPool, q: &str, limit: i64) -> AppResult<Vec<SearchResult>> {
    let rows: Vec<(String, String, String, String, f32)> = sqlx::query_as(
        r#"
        select si.url,
               si.type,
               si.title,
               left(coalesce(nullif(si.body, ''), si.title), 240) as snippet,
               similarity(si.title, $1) as rank
        from search_index si
        where si.title % $1
        order by rank desc, si.updated_at desc
        limit $2
        "#,
    )
    .bind(q)
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(to_results(rows))
}

/// Autocomplete suggestions: substring/trigram matches on the title, prefix
/// matches ranked first.
pub async fn suggest(db: &PgPool, q: &str, limit: i64) -> AppResult<Vec<Suggestion>> {
    Ok(sqlx::query_as(
        r#"select url, title
           from search_index
           where title ilike ('%' || $1 || '%') or title % $1
           order by (title ilike ($1 || '%')) desc, similarity(title, $1) desc, title asc
           limit $2"#,
    )
    .bind(q)
    .bind(limit)
    .fetch_all(db)
    .await?)
}

// ---------------------------------------------------------------------------
// Settings + synonym expansion
// ---------------------------------------------------------------------------

pub async fn get_settings(db: &PgPool) -> AppResult<SearchSettings> {
    let (title_weight, body_weight, synonyms): (f32, f32, sqlx::types::Json<Vec<Vec<String>>>) =
        sqlx::query_as(
            r#"insert into search_settings (id) values ('settings')
               on conflict (id) do update set id = 'settings'
               returning title_weight, body_weight, synonyms"#,
        )
        .fetch_one(db)
        .await?;
    Ok(SearchSettings {
        title_weight,
        body_weight,
        synonyms: synonyms.0,
    })
}

pub async fn update_settings(db: &PgPool, settings: &SearchSettings) -> AppResult<()> {
    sqlx::query(
        r#"insert into search_settings (id, title_weight, body_weight, synonyms)
           values ('settings', $1, $2, $3)
           on conflict (id) do update set
               title_weight = excluded.title_weight,
               body_weight = excluded.body_weight,
               synonyms = excluded.synonyms"#,
    )
    .bind(settings.title_weight)
    .bind(settings.body_weight)
    .bind(sqlx::types::Json(&settings.synonyms))
    .execute(db)
    .await?;
    Ok(())
}

/// Build a `to_tsquery`-safe OR expansion of the query's synonyms. Every query
/// word that appears in a synonym group ORs in that group's other terms;
/// multi-word synonyms become `<->` phrase queries. Returns "" when nothing
/// matches — the caller treats "" as "no expansion".
fn synonym_expansion(q: &str, groups: &[Vec<String>]) -> String {
    let words = tokenize(q);
    if words.is_empty() || groups.is_empty() {
        return String::new();
    }
    let mut terms: Vec<String> = Vec::new();
    for group in groups {
        let group_matches = group.iter().any(|t| words.contains(&t.to_lowercase()));
        if !group_matches {
            continue;
        }
        for t in group {
            if let Some(term) = tsquery_term(t) {
                let wrapped = format!("({term})");
                if !terms.contains(&wrapped) {
                    terms.push(wrapped);
                }
            }
        }
    }
    terms.join(" | ")
}

fn tokenize(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Render a synonym term as one `to_tsquery` lexeme, or a `<->` phrase when it
/// is multi-word.
fn tsquery_term(t: &str) -> Option<String> {
    let parts = tokenize(t);
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" <-> "))
    }
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

#[cfg(test)]
mod tests {
    use super::synonym_expansion;

    fn groups() -> Vec<Vec<String>> {
        vec![
            vec!["haus".into(), "gebäude".into()],
            vec!["auto".into(), "pkw".into(), "wagen".into()],
        ]
    }

    #[test]
    fn expands_matching_group_only() {
        let e = synonym_expansion("altes haus", &groups());
        assert!(e.contains("(haus)"));
        assert!(e.contains("(gebäude)"));
        assert!(e.contains(" | "));
        assert!(!e.contains("pkw"));
    }

    #[test]
    fn no_expansion_when_nothing_matches() {
        assert_eq!(synonym_expansion("fahrrad", &groups()), "");
        assert_eq!(synonym_expansion("haus", &[]), "");
        assert_eq!(synonym_expansion("", &groups()), "");
    }

    #[test]
    fn multiword_synonym_becomes_phrase() {
        let g = vec![vec!["kfz".into(), "kraft fahrzeug".into()]];
        let e = synonym_expansion("mein KFZ", &g);
        assert!(e.contains("(kraft <-> fahrzeug)"));
        assert!(e.contains("(kfz)"));
    }
}
