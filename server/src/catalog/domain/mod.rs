//! Pure catalog domain — the uninterpreted content tree, the browsable catalog it walks into,
//! lenient frontmatter, and pure navigation. NO axum / tokio / sqlx / reqwest here — an
//! infrastructure import in this tree means a port was skipped.

pub mod catalog;
pub mod component_doc;
pub mod content_tree;
pub mod frontmatter;
pub mod lesson;
pub mod resolver;
pub mod walker;
