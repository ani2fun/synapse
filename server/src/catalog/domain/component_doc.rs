//! LikeC4 component tutorial docs — co-located `_c4-docs/<leaf>.md` sidecars next to the lesson.
//! Has its own `technology` field, which is why it does not reuse `LessonFrontmatter`.

use crate::catalog::domain::frontmatter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentDoc {
    pub title: Option<String>,
    pub kind: Option<String>,
    pub technology: Option<String>,
    pub body: String,
}

impl ComponentDoc {
    /// Lenient: an absent fence leaves all metadata `None` and the whole source as body.
    pub fn parse(raw: &str) -> Self {
        let (fields, body) = frontmatter::fields_and_body(raw);
        Self {
            title: fields.get("title").cloned(),
            kind: fields.get("kind").cloned(),
            technology: fields.get("technology").cloned(),
            body,
        }
    }
}

#[cfg(test)]
mod tests;
