//! Which renderer each structure dispatches to. Several structures deliberately SHARE one — queue
//! with deque, tree with segment tree — so the mapping is pinned here rather than assumed.

use super::*;

#[test]
fn the_cells_family_covers_exactly_the_plain_row_shapes() {
    let cells: Vec<_> = VizStructure::ALL
        .iter()
        .filter(|s| RenderFamily::of(**s) == RenderFamily::Cells)
        .collect();
    assert_eq!(cells.len(), 3, "array · bitset · fenwick");
    for s in [VizStructure::Array, VizStructure::Bitset, VizStructure::Fenwick] {
        assert_eq!(RenderFamily::of(s), RenderFamily::Cells);
    }
}

#[test]
fn queue_and_deque_share_the_queue_strip() {
    assert_eq!(RenderFamily::of(VizStructure::Queue), RenderFamily::Queue);
    assert_eq!(RenderFamily::of(VizStructure::Deque), RenderFamily::Queue);
}

#[test]
fn stack_and_callstack_draw_as_stack() {
    assert_eq!(RenderFamily::of(VizStructure::Stack), RenderFamily::Stack);
    assert_eq!(RenderFamily::of(VizStructure::Callstack), RenderFamily::Stack);
}

#[test]
fn tree_and_segment_tree_share_the_tree_renderer() {
    assert_eq!(RenderFamily::of(VizStructure::Tree), RenderFamily::Tree);
    assert_eq!(RenderFamily::of(VizStructure::SegmentTree), RenderFamily::Tree);
}

#[test]
fn the_bespoke_families_dispatch_exactly() {
    assert_eq!(RenderFamily::of(VizStructure::Heap), RenderFamily::HeapDual);
    assert_eq!(RenderFamily::of(VizStructure::List), RenderFamily::LinkedList);
    assert_eq!(RenderFamily::of(VizStructure::Skiplist), RenderFamily::Chain);
    assert_eq!(RenderFamily::of(VizStructure::Graph), RenderFamily::Force);
    assert_eq!(RenderFamily::of(VizStructure::Hashmap), RenderFamily::Buckets);
    assert_eq!(RenderFamily::of(VizStructure::UnionFind), RenderFamily::Forest);
    assert_eq!(RenderFamily::of(VizStructure::Trie), RenderFamily::Trie);
    assert_eq!(RenderFamily::of(VizStructure::Grid), RenderFamily::Grid);
}
