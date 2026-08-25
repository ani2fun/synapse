//! Parsing a structure token into a structure and its optional root. Names may be dotted or
//! kebab-cased, and an unknown token reads as `None` rather than a guess — legacy spellings
//! included, because a silent wrong structure draws a confidently wrong picture.

#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn parses_a_bare_structure_and_structure_root() {
    assert_eq!(VizStructure::parse("stack"), Some((VizStructure::Stack, None)));
    assert_eq!(
        VizStructure::parse("array:nums"),
        Some((VizStructure::Array, Some("nums".to_owned())))
    );
}

#[test]
fn preserves_a_dotted_root() {
    assert_eq!(
        VizStructure::parse("list:self.head"),
        Some((VizStructure::List, Some("self.head".to_owned())))
    );
}

#[test]
fn handles_kebab_case_names() {
    assert_eq!(
        VizStructure::parse("union-find:p").map(|(s, _)| s),
        Some(VizStructure::UnionFind)
    );
    assert_eq!(
        VizStructure::from_name("segment-tree"),
        Some(VizStructure::SegmentTree)
    );
}

#[test]
fn an_empty_root_after_the_colon_reads_as_no_root() {
    assert_eq!(VizStructure::parse("tree:"), Some((VizStructure::Tree, None)));
}

#[test]
fn unknown_tokens_are_none_including_migrated_legacy_names() {
    assert_eq!(VizStructure::parse("frobnicate"), None);
    assert_eq!(VizStructure::from_name("binary-tree"), None);
    assert_eq!(VizStructure::from_name("linked-list"), None);
}

#[test]
fn maps_each_structure_to_its_geometry_family() {
    assert_eq!(VizStructure::Array.layout(), LayoutKind::Cells);
    assert_eq!(VizStructure::Callstack.layout(), LayoutKind::Cells);
    assert_eq!(VizStructure::Grid.layout(), LayoutKind::Grid);
    assert_eq!(VizStructure::Tree.layout(), LayoutKind::Tree);
    assert_eq!(VizStructure::Heap.layout(), LayoutKind::Tree);
    assert_eq!(VizStructure::List.layout(), LayoutKind::Chain);
    assert_eq!(VizStructure::Graph.layout(), LayoutKind::Graph);
    assert_eq!(VizStructure::Hashmap.layout(), LayoutKind::Graph);
}

#[test]
fn token_round_trips_through_from_name_for_every_structure() {
    for s in VizStructure::ALL {
        assert_eq!(VizStructure::from_name(s.token()), Some(s), "{s:?}");
    }
}
