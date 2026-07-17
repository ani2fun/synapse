//! The placeholder decode contract (oracle: `RunnableBlocks.scala`, pure half). The pipeline
//! emits `<div class="workbench" data-variants="<uri-encoded JSON>">`; the JSON is
//! `[{lang, source, viz?}]`. Languages are trimmed, blank-lang variants dropped, and an empty
//! list means the block is skipped. URI decoding is the view's job (it needs JS) — this stays
//! native-testable.

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RawVariant {
    lang: String,
    source: String,
    #[serde(default)]
    viz: Option<String>,
}

/// One language rendition of a runnable block (oracle: shared `CodeVariant` + the
/// positionally-paired `VizHints`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    pub language: String,
    pub source: String,
    /// The fence's `viz=<structure>[:<root>]` hint, raw (parsed on use).
    pub viz: Option<String>,
}

/// Visualise needs a Python or Java variant with a `viz=` hint (oracle:
/// `WorkbenchLogic.canVisualise`).
pub fn can_visualise(variant: &Variant) -> bool {
    variant.viz.is_some() && matches!(variant.language.to_lowercase().as_str(), "python" | "java")
}

/// Decode the (already URI-decoded) `data-variants` JSON. Malformed or empty → `None`
/// (the block is skipped, never a crash — authored content must not take the reader down).
pub fn parse_variants(json: &str) -> Option<Vec<Variant>> {
    let raw: Vec<RawVariant> = serde_json::from_str(json).ok()?;
    let variants: Vec<Variant> = raw
        .into_iter()
        .map(|v| Variant {
            language: v.lang.trim().to_owned(),
            source: v.source,
            viz: v.viz,
        })
        .filter(|v| !v.language.is_empty())
        .collect();
    if variants.is_empty() { None } else { Some(variants) }
}

/// Display name for a fence alias (oracle: `WorkbenchLogic.displayLang`).
pub fn display_lang(alias: &str) -> String {
    match alias.to_lowercase().as_str() {
        "cpp" | "c++" => "C++".to_owned(),
        "csharp" => "C#".to_owned(),
        "rs" => "Rust".to_owned(),
        "kt" => "Kotlin".to_owned(),
        "js" => "JavaScript".to_owned(),
        "ts" => "TypeScript".to_owned(),
        other => {
            let mut chars = other.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        }
    }
}

/// Seed the values grid from an authored case (oracle: `WorkbenchLogic.seedValues`).
pub fn seed_values(
    spec: &synapse_shared::execution::TestSpec,
    case_index: usize,
) -> std::collections::BTreeMap<String, String> {
    spec.cases
        .get(case_index)
        .map(|case| case.args.clone())
        .unwrap_or_default()
}

/// The active case's expected stdout, when declared.
pub fn expected_for(spec: &synapse_shared::execution::TestSpec, case_index: usize) -> Option<String> {
    spec.cases.get(case_index).and_then(|case| case.expected.clone())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn decodes_single_and_adjacent_variants_in_order() {
        let json =
            r#"[{"lang":"python","source":"print(1)"},{"lang":"java","source":"class S {}","viz":"array"}]"#;
        let variants = parse_variants(json).unwrap();
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0].language, "python");
        assert_eq!(variants[1].source, "class S {}");
    }

    #[test]
    fn trims_langs_and_drops_blank_ones() {
        let json = r#"[{"lang":"  py  ","source":"a"},{"lang":"   ","source":"b"}]"#;
        let variants = parse_variants(json).unwrap();
        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].language, "py");
    }

    #[test]
    fn malformed_or_empty_means_skip() {
        assert_eq!(parse_variants("not json"), None);
        assert_eq!(parse_variants("[]"), None);
        assert_eq!(parse_variants(r#"[{"lang":" ","source":"x"}]"#), None);
    }

    #[test]
    fn display_names_read_well() {
        assert_eq!(display_lang("cpp"), "C++");
        assert_eq!(display_lang("python"), "Python");
        assert_eq!(display_lang("rs"), "Rust");
        assert_eq!(display_lang("kt"), "Kotlin");
        assert_eq!(display_lang("js"), "JavaScript");
    }
}
