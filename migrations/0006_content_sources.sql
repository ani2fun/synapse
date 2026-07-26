-- WHICH REPOSITORIES FEED THE LIBRARY. The primary checkout arrives by git-sync and is wired in
-- code; every other book is a repository registered here from the admin panel, fetched over the
-- GitHub API, and merged into the same catalog. Registering a repo is a row, not a redeploy —
-- which is the whole point, because the app cannot add a sync sidecar to itself at runtime.
--
-- `id` is what a lesson's file reference points back through to reach the right checkout, and it
-- names the cache directory on disk, so it is slug-shaped and immutable. It is derived from the
-- repository name rather than typed: one less field to get wrong, and it stays deterministic.
--
-- `grouping` is the '/'-joined category slug path the book grafts under ('' = the top level), and
-- `sort_order` its position within that group. Both live HERE rather than in the repository's
-- book.json deliberately: what a book IS travels with the book, where it SITS is the platform's
-- business, and moving one should not need a content push. book.json's own `order` remains the
-- fallback when sort_order is null.
--
-- What does NOT live here is the book's slug. That is the URL, and it stays in book.json where the
-- content can own it — deriving it from a repository name would let a rename silently move every
-- lesson and orphan the readership and progress rows keyed on those paths.
create table content_source (
    id             text        primary key,
    repo           text        not null unique,
    branch         text        not null default 'main',
    grouping       text        not null default '',
    sort_order     int,
    enabled        boolean     not null default true,
    -- Sync state, written by the fetch loop and read by the admin panel. A source that has never
    -- landed has a null sha; one that failed keeps its last good sha AND the error, so a broken
    -- push degrades to stale content rather than to an empty book.
    last_sha       text,
    last_synced_at timestamptz,
    last_error     text,
    created_at     timestamptz not null default now(),
    updated_at     timestamptz not null default now()
);

-- The fetch loop and the catalog both want "what should be mounted, in order" on a hot path.
create index content_source_enabled_order on content_source (enabled, sort_order, id);
