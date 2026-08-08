-- One row per indexed URL. `tsv` is a stored generated column so the full-text
-- vector is always in sync with title/body and can back a GIN index. The
-- German config (stemming + stop words) is named explicitly because the
-- two-argument `to_tsvector(regconfig, text)` form is IMMUTABLE — required for
-- a generated column — whereas the one-argument form is not.
--
-- `source` separates rows written by the content reindex (`db`, from the CMS
-- `page` table — title + description, covers every page instantly) from rows
-- enriched by the rewriter (`rendered`, full visible page text, added/updated
-- as pages are actually viewed). Keeping them apart lets a reindex prune its
-- own stale rows without deleting rendered rows for posts/events.
create table if not exists search_index (
    url text primary key,
    page_id int,
    type text not null default 'page',
    source text not null default 'db',
    lang text,
    title text not null default '',
    body text not null default '',
    tsv tsvector generated always as (
        setweight(to_tsvector('german', coalesce(title, '')), 'A') ||
        setweight(to_tsvector('german', coalesce(body, '')), 'B')
    ) stored,
    updated_at timestamptz not null default now()
);

create index if not exists search_index_tsv_idx on search_index using gin (tsv);
create index if not exists search_index_type_idx on search_index (type);
create index if not exists search_index_source_idx on search_index (source);

-- Every visitor search, so the dashboard can surface top queries and — the
-- content-gap insight — queries that returned nothing.
create table if not exists search_query_log (
    id bigint generated always as identity primary key,
    q text not null,
    results_count int not null,
    created_at timestamptz not null default now()
);

create index if not exists search_query_log_created_idx on search_query_log (created_at desc);
create index if not exists search_query_log_noresult_idx on search_query_log (results_count);
