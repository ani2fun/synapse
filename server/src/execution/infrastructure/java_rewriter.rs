//! Java entrypoint normaliser (oracle: `JavaSourceRewriter.scala`). The sandbox writes
//! `Main.java` and runs `java Main`, but DSA lessons write `class Solution` — rename the first
//! top-level class (and its self-references) to `Main`, unless a `Main` already exists or the
//! source is a tracer harness (the sentinel skip, oracle Phase-4).

use std::sync::LazyLock;

use regex::Regex;

use crate::execution::domain::Language;

/// The tracer harness's first line (oracle: `TracerMarks.JavaSentinel`) — traced Java already
/// defines `Main`, so it must pass through untouched.
pub(crate) const JAVA_TRACER_SENTINEL: &str = "// __SYNAPSE_TRACER__";

static HAS_MAIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bclass\s+Main\b").unwrap_or_else(|e| unreachable!("static regex: {e}")));

/// First TOP-LEVEL class: anchored `^` so indented/nested classes never match.
static TOP_LEVEL_CLASS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^(?:public\s+)?(?:(?:final|abstract|static|sealed|non-sealed)\s+)*class\s+(\w+)\b")
        .unwrap_or_else(|e| unreachable!("static regex: {e}"))
});

/// The source the sandbox should compile: Java gets normalised, everything else (and traced
/// Java) passes through.
pub(crate) fn effective_source(language: Language, source: &str) -> String {
    if language == Language::Java && !source.starts_with(JAVA_TRACER_SENTINEL) {
        normalize_entrypoint(source)
    } else {
        source.to_owned()
    }
}

fn normalize_entrypoint(source: &str) -> String {
    if HAS_MAIN.is_match(source) {
        return source.to_owned(); // collision guard: an authored Main wins
    }
    let Some(captures) = TOP_LEVEL_CLASS.captures(source) else {
        return source.to_owned();
    };
    let Some(name) = captures.get(1).map(|m| m.as_str()) else {
        return source.to_owned();
    };
    if name == "Main" {
        return source.to_owned();
    }
    // Word-boundary replace catches `new Solution()`, `Solution.helper()` — not `SolHelper`.
    let Ok(word) = Regex::new(&format!(r"\b{}\b", regex::escape(name))) else {
        return source.to_owned();
    };
    word.replace_all(source, "Main").into_owned()
}

#[cfg(test)]
#[path = "java_rewriter_tests.rs"]
mod tests;
