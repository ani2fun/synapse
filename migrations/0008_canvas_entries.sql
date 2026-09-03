-- Saved Algorithm Design Canvas entries — the reader's PLAN for a problem, the way `submissions`
-- stores their CODE for it. One row per save: a timestamped snapshot the reader can re-read,
-- export or delete, keyed by the same opaque Keycloak `sub` both of those tables use.
--
-- The body is jsonb and NOT decomposed into columns: the eight areas plus a variable-length list
-- of ideas are one authored document, always read and written whole, and never queried across.
-- A column per area would buy a filter nothing asks for and a migration every time the canvas
-- gains a section.
--
-- Nothing derivable is stored. The entry's title (the first line of Problem), its filled count
-- and its best complexity are all computed from the body by the client that renders them — a
-- stored copy is a copy that can disagree with the body it claims to describe.
create table canvas_entries (
    id          uuid primary key,
    user_id     text        not null,
    lesson_path text        not null,
    body        jsonb       not null,
    created_at  timestamptz not null default now()
);

-- The only read is "this caller's entries for this problem, newest first" — see CanvasStore::list_for.
create index canvas_entries_owner_recency
    on canvas_entries (user_id, lesson_path, created_at desc);
