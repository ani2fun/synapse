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
mod tests;
