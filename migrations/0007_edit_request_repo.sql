-- WHICH REPOSITORY A PROPOSAL WAS OPENED AGAINST.
--
-- Until satellites, there was exactly one content repository, so the branch alone identified a
-- proposal and `branch` could be globally unique. With a book per repository neither holds: two
-- repositories can carry the same branch name legitimately (`edit/ada/java/intro` in the monorepo
-- and in the guide repo are different branches on different forges), and a revision has to know
-- which one to commit to.
--
-- Recorded rather than re-derived from the page's current source, because those can disagree. If a
-- book migrates out of the monorepo while a pull request is open, the open branch still lives in
-- the repository it was opened against — following the page's new source would commit into the
-- wrong repository, or fail looking for a branch that was never there.
alter table content_edit_request add column repo text;

-- Every existing row predates satellites, so it can only have targeted the monorepo.
update content_edit_request set repo = 'ani2fun/synapse-content' where repo is null;
alter table content_edit_request alter column repo set not null;

-- The uniqueness that actually holds: one proposal per branch PER REPOSITORY.
alter table content_edit_request drop constraint if exists content_edit_request_branch_key;
alter table content_edit_request add constraint content_edit_request_repo_branch_key unique (repo, branch);
