# Development

Developer notes for the `site-search` Neleto plugin. For the user-facing
description, see [readme.md](readme.md).

> Status: **P1 + P2 + P3** of the [plugin roadmap](../plugin-cli/docs/plugin-roadmap.md)
> — P1: reindex + `/search` page + search-bar component + Postgres FTS. P2: live
> rewriter indexing + query logging + dashboard cards. P3: autocomplete +
> `pg_trgm` typo tolerance + query-time weighting/synonyms.

## How it works

The plugin runs as a standard Neleto plugin subprocess and reads two databases
injected by the backend: `DATABASE_URL` (its own search index + query log) and
`CMS_DATABASE_URL` (the CMS content). The index is filled from two sources:

1. **Content reindex** — reads every public page from the CMS `page` table and
   seeds the index with its title + description. Complete and instant; covers
   every page. Triggered by the **Reindex pages** button (`POST /api/reindex`).
   Prunes rows for pages that no longer exist.
2. **Rewriter enrichment** — the `rewriter` hook receives the full HTML of every
   page render and indexes its **full visible text** in a detached task (so it
   never adds page latency), keyed on the page's canonical / `og:url`. This is
   also how **posts and events** get indexed — at their real URLs, with full
   text — since they're not in the `page` table.

Rows carry a `source` (`db` vs `rendered`) so the two never fight: a reindex only
prunes its own `db` rows, and never downgrades a row the rewriter has enriched
with full text.

## Search

PostgreSQL full-text search with the **`german`** config (stemming + stop words).
The `tsv` column is a **stored generated column** — `setweight(title, 'A') ||
setweight(body, 'B')` — backed by a GIN index. Queries use
`websearch_to_tsquery` (so visitors can use quotes / `-exclude` naturally) ranked
by `ts_rank`, with `ts_headline` snippets.

**Weighting & synonyms (P3)** are applied at query time, so neither needs a
reindex. `ts_rank` takes a `{D,C,B,A}` weight array built from the configurable
`title_weight` / `body_weight` (`search_settings`). Synonyms are expanded in Rust
(`synonym_expansion`): any query word in a configured group ORs the group's other
terms into the tsquery via `websearch_to_tsquery(…) || to_tsquery(…)` (multi-word
synonyms become `<->` phrases). Both are edited in the admin panel.

**Typo tolerance & autocomplete (P3)** use `pg_trgm` (a GIN trigram index on
`title`). When exact FTS finds nothing, `search()` falls back to
`similarity(title, q)` (the `%` operator). Autocomplete is a **public
`/search/suggest`** `pages` route returning JSON (substring + trigram on titles,
prefix matches first); the search-bar component's `script.js` debounces calls to
it and renders a keyboard-navigable dropdown.

The `german` config must exist in the target Postgres (it ships with the default
install). The two-argument `to_tsvector('german', …)` form is used deliberately —
it's `IMMUTABLE`, which the generated column requires; the one-argument form is
not.

## The `/search` page

Registered as a `pages` route with `allow_select_layout: true`. When a visitor
hits `/search?q=…`, Neleto renders the operator's chosen layout down to a
`<slot></slot>` placeholder and **proxies the request to the plugin as a POST**
whose body is that rendered layout (the `?q=` query is preserved on the URL). The
handler runs the search and splices the results into the slot. If no layout is
selected the body is empty and the handler returns a minimal standalone page, so
the route always works — including for local `GET /search?q=…` testing.

Snippets from `ts_headline` are wrapped with `[[hl]]…[[/hl]]` sentinels (not HTML)
so the whole snippet can be HTML-escaped and only *then* turned into `<mark>`
tags — page text can never inject markup.

## Manifest hooks

| Hook | Route | Purpose |
|---|---|---|
| `pages` | `/search` | Public results page (layout-selectable) |
| `pages` | `/search/suggest` | Public autocomplete JSON (no layout) |
| `rewriter` | `/rewriter` | Live full-text indexing on every page render |
| `components` | `components/search-bar` | Search box (submits to `/search`) + autocomplete `script.js` |
| `dashboard_cards` | `/dashboard/top-queries`, `/dashboard/no-results` | Server-rendered analytics cards |
| `api` | `/api/stats`, `/api/reindex`, `/api/settings` | Admin data + reindex + weighting/synonyms |
| `ui` | `/ui` | Admin panel (stats + reindex + query tables + settings) |

## Theming

Dashboard cards and the admin UI render in **same-origin iframes** themed with
Nuxt UI CSS variables (`--ui-bg`, `--ui-text`, `--ui-primary`, …). Neleto passes
no theme info into the iframe, so `templates/theme_head.html` (inlined into the
cards, and duplicated inline in the admin UI) reads the parent document and
copies its resolved `--ui-*` colors and `.dark` class onto the iframe root, with
standalone fallbacks when the parent isn't reachable.

## Running locally

```sh
cp .env.example .env      # point DATABASE_URL + CMS_DATABASE_URL at local Postgres
just dev                  # cargo watch -x run   (needs cargo-watch)
# or:
cargo run
```

The admin UI is a single static file (`ui/dist/index.html`) — no build step. It
derives the API base path from its own URL, so it works behind the Neleto proxy
at `/api/rest/plugins/site-search/…`.

Test a search without the CMS wiring: `curl 'localhost:3000/search?q=haus'`
returns the standalone results page.

### Test & build

```sh
cargo test                # indexer + search-page unit tests
just build                # release build for the x86_64-unknown-linux-gnu target
plugin-cli package        # produce the installable archive
```

## Layout

```
src/
  main.rs        axum app + routing
  lib.rs         AppState, DB + template setup
  model.rs       CMS rows, PageContent, search + stats read models
  indexer.rs     pure HTML → (url, title, text) extraction + unit tests
  reindex.rs     content reindex orchestration (CMS pages → index)
  rewriter.rs    rewriter hook (background full-text indexing)
  database.rs    CMS reads, index upserts, FTS search (weights/synonyms/fuzzy),
                 autocomplete, query log, stats, settings
  api/           search page + suggest / admin (stats, reindex) / dashboard / settings
templates/       minijinja dashboard-card fragments
components/search-bar/  search box + autocomplete script.js (submits to /search)
ui/dist/         static admin panel
migrations/      plugin schema (search_index, search_query_log)
```

## Roadmap (still open — post-P3)

- **Language filtering** for multilingual sites (the `lang` column is already
  captured from the rendered `<html lang>`, just not yet used to filter, and
  `/search` doesn't yet accept a `lang` param).
- **Reconcile rendered rows** — drop indexed post/event URLs that now 404
  (rendered rows currently aren't pruned by a reindex).
- A **results component** (`search_results` helper) to embed results inline in a
  layout, in addition to the dedicated `/search` page.
- **Body trigram index** — the typo fallback and autocomplete match on `title`
  only; add a `body` trigram index if fuzzy body matching is wanted.
