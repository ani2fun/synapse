-- WHO MAY LISTEN. Reading a lesson aloud runs the speech engine on the cluster's own CPU, and
-- one engine instance only just keeps ahead of one listener, so this is a grant of shared compute
-- in the same sense the submit allowlist is — but a DIFFERENT grant. Someone who may save
-- attempts has not thereby been given the speech engine, and revoking one must never silently
-- revoke the other. Keyed by the lowercase IdP username, exactly like its siblings, because the
-- verifier canonicalises once and the compare is apples-to-apples.
create table listen_allowlist (
    username   text        primary key,
    note       text,
    granted_at timestamptz not null default now()
);

-- Dev realm seed (throwaway realm-file user). `test1` is deliberately NOT seeded: the e2e suite
-- proves the gate with a signed-in reader who is refused.
insert into listen_allowlist (username, note) values ('tester', 'dev realm seed');
