-- A reader's notes for a problem — the Notes tab beside the code editor. ONE living document per
-- account per problem, not a history: unlike `canvas_entries`, where each save is a snapshot the
-- reader chose to keep, a note is a scratchpad that is simply the latest thing they wrote. So the
-- key IS (user_id, lesson_path) and a save is an upsert.
--
-- The body is plain text (markdown the client renders); nothing is parsed or derived server-side.
-- An empty note is not stored: saving "" deletes the row, so a problem opened and never written in
-- leaves nothing behind.
create table problem_notes (
    user_id     text        not null,
    lesson_path text        not null,
    body        text        not null,
    updated_at  timestamptz not null default now(),
    primary key (user_id, lesson_path)
);
