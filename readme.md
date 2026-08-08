# Site Search

Add a real search box to your Neleto site. **Site Search** indexes your pages and
lets visitors find what they're looking for — fast, typo-friendly, and entirely
self-hosted. No Algolia, no external service, no per-search fees. Everything runs
inside your Neleto instance on PostgreSQL full-text search.

Drop the **search bar** component onto any page, and search results appear on a
`/search` page that uses whichever layout you choose — so it looks like the rest
of your site. Two dashboard cards show you what people search for, and — just as
useful — what they search for and *don't* find.

## Why you'll want it

- **Help visitors find things.** A proper site search turns "I gave up" into "I
  found it" — especially on content-heavy sites.
- **German-aware.** Uses PostgreSQL's German text search, so *Häuser* matches
  *Haus* and common stop words are ignored.
- **See what people want.** The **Top Searches** card shows your most popular
  queries; **Searches Without Results** reveals the content your visitors expect
  but you don't have yet — a ready-made content to-do list.
- **Always fresh.** Pages are indexed automatically as they're visited, and a
  one-click **Reindex** covers everything at once.
- **Zero external dependencies.** No API keys, no third-party service, no data
  leaving your server.
- **Zero slowdown.** Indexing happens quietly in the background and never slows
  a page down for your visitors.

## How to use it

1. **Install** Site Search from the plugin store.
2. Add the **Search Bar** component to your header, homepage, or a dedicated
   search page.
3. Open the **Site Search** admin panel and click **Reindex pages** so every
   page is searchable immediately (new and updated pages are picked up on their
   own as they're viewed).
4. Choose a **layout** for the `/search` results page (Neleto asks which layout
   to use), so results match your site's look.
5. Watch the **dashboard cards** fill in as visitors search.

## What gets searched

- **Every public page** — title and description immediately after a reindex, and
  the full visible page text once the page has been viewed at least once.
- **Blog posts and events** become searchable as they're visited, indexed at
  their real URLs with their full text.

Draft and deleted pages are never indexed, and pages removed from your site drop
out of the index automatically on the next reindex.

## Who can use it

- **Admins & developers** can reindex and see all search analytics.
- **Editors** can view the search analytics to spot content gaps.

## Good to know

- **Nothing to configure.** Install, add the search bar, reindex — done.
- **The results page is yours.** Pick any layout for `/search`; the plugin only
  fills in the results.
- **Matches your theme.** The admin panel and dashboard cards follow your
  dashboard's colors and dark mode.

---

Building on or contributing to the plugin? See [DEVELOPMENT.md](DEVELOPMENT.md).
