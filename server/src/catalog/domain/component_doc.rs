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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_metadata_and_body() {
        let doc =
            ComponentDoc::parse("---\ntitle: Reader\nkind: component\ntechnology: Laminar\n---\nThe body.");
        assert_eq!(doc.title.as_deref(), Some("Reader"));
        assert_eq!(doc.kind.as_deref(), Some("component"));
        assert_eq!(doc.technology.as_deref(), Some("Laminar"));
        assert_eq!(doc.body, "The body.");
    }

    #[test]
    fn absent_fence_means_all_none_and_full_body() {
        let doc = ComponentDoc::parse("Just prose.");
        assert_eq!(doc.title, None);
        assert_eq!(doc.kind, None);
        assert_eq!(doc.technology, None);
        assert_eq!(doc.body, "Just prose.");
    }
}
