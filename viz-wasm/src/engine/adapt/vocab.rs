//! The adapter's name heuristics, in one documented place, pinned exactly so the
//! cortex-goldens fixtures compare byte-for-byte. Adapter concerns —
//! kept here, not in the authored `vocabulary`. Known quirk carried on purpose: the
//! single-letter breadth of `INDEX_NAMES` (`l`/`r`/`m`/…) can promote a coincidentally-named
//! integer local to an array-index cursor.

/// Priority order for an instance's "primary" display value; the rest become meta rows.
pub const VALUE_FIELDS: [&str; 5] = ["val", "value", "data", "key", "item"];

/// The label shown for a bare reference cell/field.
pub const REF_LABEL: &str = "·";

/// Integer locals with these names become array-index cursors (when in range).
pub fn is_index_name(name: &str) -> bool {
    matches!(
        name,
        "i" | "j"
            | "k"
            | "l"
            | "r"
            | "m"
            | "lo"
            | "hi"
            | "mid"
            | "low"
            | "high"
            | "left"
            | "right"
            | "start"
            | "end"
            | "first"
            | "last"
            | "p"
            | "q"
            | "pivot"
            | "idx"
            | "index"
            | "pos"
            | "slow"
            | "fast"
            | "read"
            | "write"
            | "front"
            | "back"
            | "top"
    )
}

/// Layout-kind names the authored override may force (incl. the legacy names the goldens
/// still carry).
pub fn is_known_layout_kind(name: &str) -> bool {
    matches!(
        name,
        "tree-binary"
            | "list-single"
            | "list-double"
            | "hashmap"
            | "array-1d"
            | "array-2d"
            | "graph-generic"
            | "binary-tree"
            | "linked-list"
            | "array"
            | "grid"
            | "graph"
    )
}

const HELPER_FN_PREFIXES: [&str; 3] = ["from_", "to_", "build_"];
const JAVA_BUILDER_PREFIXES: [&str; 5] = ["from", "to", "build", "make", "create"];

/// A constructor / builder frame the projection drops (setup noise isn't a step).
pub fn is_helper_frame(fn_name: &str) -> bool {
    fn_name == "__init__"
        || fn_name == "<init>"
        || HELPER_FN_PREFIXES.iter().any(|p| fn_name.starts_with(p))
        || is_java_builder_frame(fn_name)
}

// Java camelCase builder: `fromX`/`toX`/… — gated on the next char being uppercase so
// `from`/`total` aren't caught.
fn is_java_builder_frame(fn_name: &str) -> bool {
    JAVA_BUILDER_PREFIXES.iter().any(|p| {
        fn_name.len() > p.len()
            && fn_name.starts_with(p)
            && fn_name[p.len()..].chars().next().is_some_and(char::is_uppercase)
    })
}
