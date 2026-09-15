-- WHO MAY READ A REGISTERED REPOSITORY. Every source used to be public by construction: the
-- catalog took no identity, and a book was served to whoever asked for its URL. A private
-- repository — one the server's GitHub token can read but the world cannot — needs the other
-- half: a reader list, and every read path (index, search, lesson, sitemap) consulting it.
--
-- `visibility` lives on the source row rather than on the book, because it is the REGISTRATION
-- that decides who a repository is for; a book.json is authored by the repository and could not
-- be trusted to declare itself public. The readers are a child table keyed on the username in the
-- same canonical form the submit and content-editor allowlists use (trimmed, lowercased), so one
-- spelling rule governs every grant in the system.
alter table content_source
    add column visibility text not null default 'public'
        check (visibility in ('public', 'private'));

create table content_source_reader (
    source_id  text        not null references content_source(id) on delete cascade,
    username   text        not null,
    note       text,
    granted_at timestamptz not null default now(),
    primary key (source_id, username)
);
