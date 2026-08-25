//! What a reviewer actually reads: the commit message and the pull-request title and body.
//!
//! One policy is encoded here and worth stating plainly. The commit is AUTHORED by the
//! deployment's token identity, and the contributor is NAMED in the message — not set as the git
//! author. Writing someone's address into a public repository's history is not ours to do on
//! their behalf, and the attribution a reviewer needs ("who asked for this, and why") is carried
//! perfectly well by a line of prose.

/// Git's conventional subject limit — long enough to be useful in `git log --oneline`, short
/// enough not to wrap.
const SUBJECT_LIMIT: usize = 72;
const TITLE_LIMIT: usize = 100;

const NO_SUMMARY: &str = "_The contributor did not add a summary._";

/// `content: edit <path>` plus the contributor's own words and their name.
pub fn commit_message(lesson_path: &str, username: &str, summary: Option<&str>) -> String {
    let subject = elide(&format!("content: edit {lesson_path}"), SUBJECT_LIMIT);
    let summary = summary.map(str::trim).filter(|s| !s.is_empty());
    match summary {
        Some(text) => format!("{subject}\n\n{text}\n\nRequested by @{username} via Synapse.\n"),
        None => format!("{subject}\n\nRequested by @{username} via Synapse.\n"),
    }
}

pub fn pull_request_title(lesson_path: &str) -> String {
    elide(&format!("Content edit: {lesson_path}"), TITLE_LIMIT)
}

/// The body leads with the LIVE page, because the fastest way to review a prose change is to read
/// the page it changes next to the diff.
pub fn pull_request_body(
    site_url: &str,
    lesson_path: &str,
    file_path: &str,
    username: &str,
    summary: Option<&str>,
) -> String {
    let page = format!("{}/synapse/{lesson_path}", site_url.trim_end_matches('/'));
    let summary = summary
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(NO_SUMMARY);
    format!(
        "A reader proposed this change from Synapse's in-app editor.\n\
         \n\
         - **Page:** [{lesson_path}]({page})\n\
         - **File:** `{file_path}`\n\
         - **Requested by:** @{username}\n\
         \n\
         ---\n\
         \n\
         {summary}\n"
    )
}

/// Truncate on a char boundary with an ellipsis, so a deep lesson path does not produce a
/// 200-character commit subject.
fn elide(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }
    let kept: String = value.chars().take(limit.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

#[cfg(test)]
mod tests;
