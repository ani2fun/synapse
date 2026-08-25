//! Multi-board d2 walkthroughs — co-located `_d2/<fence>/<file>` sidecars next to the lesson.
//!
//! A `d2 boards` fence compiles to a TREE of boards, and a content repo's CI draws each one into
//! this directory beside the lesson that shows it (`dev-tools/render-d2.mjs`). The leading `_`
//! keeps the whole directory out of the catalog walk, the same way `_c4-docs/` stays out.
//!
//! `BoardFile` is the served-file allowlist expressed as a type: a request names a file, that
//! name is joined to a real filesystem path, and the only two things a walkthrough directory
//! legitimately holds are boards and their manifest.

/// The sidecar directory beside a lesson. Agreed with `BOARDS_DIR` in `dev-tools/d2-boards.mjs`
/// and `web/src/lib/islands/diagram/boards.ts`.
pub const BOARDS_DIR: &str = "_d2";

/// The manifest naming every board in one walkthrough.
pub const MANIFEST_FILE: &str = "boards.json";

/// What a walkthrough directory may hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardFile {
    /// One board, drawn ahead of time.
    Svg,
    /// The board graph the viewer navigates by.
    Manifest,
}

impl BoardFile {
    /// A requested filename split into its stem and kind, or `None` when it is neither shape.
    ///
    /// The stem is returned rather than discarded because the caller still has to check it is a
    /// slug — this decides only that the *extension* is one we serve.
    pub fn parse(name: &str) -> Option<(&str, Self)> {
        if let Some(stem) = name.strip_suffix(".svg") {
            return Some((stem, Self::Svg));
        }
        // Exactly one manifest name, not any `.json`: a walkthrough directory holds nothing else,
        // and an open `.json` suffix would serve whatever an author happened to leave there.
        if name == MANIFEST_FILE {
            return Some((name.trim_end_matches(".json"), Self::Manifest));
        }
        None
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Svg => "image/svg+xml",
            Self::Manifest => "application/json",
        }
    }
}

/// One board sidecar, read whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D2Board {
    pub file: BoardFile,
    pub body: String,
}

#[cfg(test)]
mod tests;
