//! Every runnable language, with its display label and the fence aliases authors write.

/// The runnable languages. Adding one here won't compile until every exhaustive `match`
/// downstream (the go-judge recipes) handles it — that is the point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Python,
    Java,
    Scala,
    C,
    Cpp,
    Go,
    Rust,
    Kotlin,
    TypeScript,
    JavaScript,
    Sql,
}

impl Language {
    pub const ALL: [Language; 11] = [
        Self::Python,
        Self::Java,
        Self::Scala,
        Self::C,
        Self::Cpp,
        Self::Go,
        Self::Rust,
        Self::Kotlin,
        Self::TypeScript,
        Self::JavaScript,
        Self::Sql,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Python => "Python 3",
            Self::Java => "Java 21 (OpenJDK)",
            Self::Scala => "Scala 3",
            Self::C => "C (GCC)",
            Self::Cpp => "C++ (GCC)",
            Self::Go => "Go",
            Self::Rust => "Rust",
            Self::Kotlin => "Kotlin",
            Self::TypeScript => "TypeScript",
            Self::JavaScript => "JavaScript (Node.js)",
            Self::Sql => "SQL (SQLite 3)",
        }
    }

    pub fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Python => &["python", "py", "python3"],
            Self::Java => &["java"],
            Self::Scala => &["scala"],
            Self::C => &["c"],
            Self::Cpp => &["cpp", "c++", "cxx"],
            Self::Go => &["go", "golang"],
            Self::Rust => &["rust", "rs"],
            Self::Kotlin => &["kotlin", "kt"],
            Self::TypeScript => &["typescript", "ts"],
            Self::JavaScript => &["javascript", "js", "node"],
            Self::Sql => &["sql", "sqlite"],
        }
    }

    /// Resolve a fence alias: trimmed, case-insensitive; blank or unknown → `None`.
    pub fn resolve(alias: &str) -> Option<Language> {
        let needle = alias.trim().to_lowercase();
        if needle.is_empty() {
            return None;
        }
        Self::ALL
            .iter()
            .copied()
            .find(|lang| lang.aliases().contains(&needle.as_str()))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn resolves_canonical_and_secondary_aliases() {
        assert_eq!(Language::resolve("python"), Some(Language::Python));
        assert_eq!(Language::resolve("py"), Some(Language::Python));
        assert_eq!(Language::resolve("c++"), Some(Language::Cpp));
        assert_eq!(Language::resolve("node"), Some(Language::JavaScript));
    }

    #[test]
    fn resolution_is_case_insensitive_and_trimmed() {
        assert_eq!(Language::resolve("  PyThOn3  "), Some(Language::Python));
        assert_eq!(Language::resolve("JAVA"), Some(Language::Java));
    }

    #[test]
    fn unknown_and_blank_resolve_to_none() {
        assert_eq!(Language::resolve("cobol"), None);
        assert_eq!(Language::resolve("   "), None);
        assert_eq!(Language::resolve(""), None);
    }

    #[test]
    fn aliases_are_globally_unique_and_round_trip() {
        let mut seen = BTreeSet::new();
        for lang in Language::ALL {
            assert!(!lang.label().is_empty());
            assert!(!lang.aliases().is_empty());
            for alias in lang.aliases() {
                assert!(seen.insert(*alias), "duplicate alias {alias}");
                assert_eq!(
                    Language::resolve(alias),
                    Some(lang),
                    "alias {alias} must round-trip"
                );
            }
        }
    }
}
