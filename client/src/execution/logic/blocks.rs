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

/// Can a judged failure's input be reproduced in the VISIBLE tests panel? Only when every
/// declared arg has a value in the failure.
///
/// A problem may be judged against a `<stem>.tests.json` sidecar the learner never sees, whose
/// arg ids need not match the authored fence's. Copying misaligned args would leave values under
/// keys with no input field, and `stdin_for` — which iterates the VISIBLE args — would then feed
/// the program something the judge never fed it. Extra keys are harmless: `stdin_for` ignores
/// them.
pub fn can_reproduce(
    spec: &synapse_shared::execution::TestSpec,
    args: &std::collections::BTreeMap<String, String>,
) -> bool {
    spec.args.iter().all(|arg| args.contains_key(&arg.id))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;

    use synapse_shared::execution::{ArgSpec, TestSpec};

    use super::*;

    fn spec_with(ids: &[&str]) -> TestSpec {
        TestSpec {
            args: ids
                .iter()
                .map(|id| ArgSpec {
                    id: (*id).to_owned(),
                    label: (*id).to_owned(),
                    tpe: "text".to_owned(),
                    placeholder: None,
                })
                .collect(),
            cases: Vec::new(),
        }
    }

    fn args_with(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn a_failure_covering_every_declared_arg_is_reproducible() {
        let spec = spec_with(&["nums", "target"]);
        assert!(can_reproduce(
            &spec,
            &args_with(&[("nums", "[1,2]"), ("target", "3")])
        ));
    }

    /// Extra keys are fine — `stdin_for` iterates the DECLARED args and ignores the rest.
    #[test]
    fn extra_keys_in_the_failure_do_not_block_it() {
        let spec = spec_with(&["nums"]);
        assert!(can_reproduce(&spec, &args_with(&[("nums", "[1]"), ("k", "9")])));
    }

    /// The case the guard exists for: a hidden sidecar suite declaring args the fence doesn't.
    #[test]
    fn a_missing_declared_arg_blocks_reproduction() {
        let spec = spec_with(&["nums", "target"]);
        assert!(!can_reproduce(&spec, &args_with(&[("nums", "[1,2]")])));
        assert!(!can_reproduce(
            &spec,
            &args_with(&[("numbers", "[1,2]"), ("target", "3")])
        ));
    }

    #[test]
    fn an_empty_failure_cannot_cover_a_declared_arg() {
        assert!(!can_reproduce(&spec_with(&["nums"]), &BTreeMap::new()));
    }

    /// A stdin-free problem declares nothing, so there is nothing to misalign.
    #[test]
    fn a_spec_with_no_args_is_vacuously_reproducible() {
        assert!(can_reproduce(&spec_with(&[]), &BTreeMap::new()));
        assert!(can_reproduce(&spec_with(&[]), &args_with(&[("stray", "1")])));
    }

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
