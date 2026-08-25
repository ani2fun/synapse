//! Tests for the Java entrypoint normaliser.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

#[test]
fn solution_and_its_self_references_become_main() {
    let source = "public class Solution {\n  public static void main(String[] a) { new Solution().go(); Solution.help(); }\n}";
    let rewritten = effective_source(Language::Java, source);
    assert!(rewritten.contains("public class Main"));
    assert!(rewritten.contains("new Main().go()"));
    assert!(rewritten.contains("Main.help()"));
    assert!(!rewritten.contains("Solution"));
}

#[test]
fn an_authored_main_passes_through() {
    let source = "class Main { }\nclass Solution { }";
    assert_eq!(effective_source(Language::Java, source), source);
}

#[test]
fn word_boundaries_protect_lookalikes() {
    let source = "class Solution { SolutionHelper h; SolHelper s; }";
    let rewritten = effective_source(Language::Java, source);
    assert!(rewritten.contains("class Main"));
    assert!(
        rewritten.contains("SolutionHelper h"),
        "prefix lookalike untouched"
    );
    assert!(rewritten.contains("SolHelper s"));
}

#[test]
fn modifier_orderings_are_recognised() {
    for source in [
        "public final class Box { }",
        "public abstract class Box { }",
        "final class Box { }",
    ] {
        assert!(
            effective_source(Language::Java, source).contains("class Main"),
            "failed for: {source}"
        );
    }
}

#[test]
fn nested_and_indented_classes_are_not_entrypoints() {
    let source = "public class Outer {\n  class Inner { }\n}";
    let rewritten = effective_source(Language::Java, source);
    assert!(rewritten.contains("class Main"), "the top-level class renames");
    assert!(rewritten.contains("class Inner"), "the nested class is untouched");
}

#[test]
fn non_java_and_traced_java_pass_through() {
    let python = "class Solution: pass";
    assert_eq!(effective_source(Language::Python, python), python);
    let traced = format!("{JAVA_TRACER_SENTINEL}\nclass Solution {{ }}");
    assert_eq!(effective_source(Language::Java, &traced), traced);
}
