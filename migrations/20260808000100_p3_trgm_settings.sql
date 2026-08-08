-- P3: typo tolerance + autocomplete + query-time weighting/synonyms.

-- Trigram matching (similarity() + the `%` operator) powers both the typo
-- fallback and autocomplete.
create extension if not exists pg_trgm;

create index if not exists search_index_title_trgm_idx
    on search_index using gin (title gin_trgm_ops);

-- Single-row settings for search behaviour. `title_weight` / `body_weight` feed
-- ts_rank's weight array at query time (no reindex needed). `synonyms` holds
-- groups of equivalent terms, e.g. [["haus","gebäude"],["auto","pkw","wagen"]];
-- when a query word appears in a group, the group's other terms are OR-ed in.
create table if not exists search_settings (
    id text primary key default 'settings',
    title_weight real not null default 1.0,
    body_weight real not null default 0.4,
    synonyms jsonb not null default '[]'::jsonb
);

insert into search_settings (id) values ('settings') on conflict (id) do nothing;
