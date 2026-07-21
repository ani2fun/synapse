//! The structure→renderer decision — the PURE
//! half of dispatch, shared so the modal and the inline widgets agree. The match is
//! exhaustive: adding a structure FORCES a family here (open/closed). Two kinds: the
//! GEOMETRIC families lay out nodes on an SVG canvas; the BESPOKE ones (flow-layout
//! HTML chrome) are re-derived widgets or composites.

use crate::engine::vocabulary::VizStructure;

/// The renderer family a structure draws with — its geometry, not its chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderFamily {
    Cells,
    Stack,
    Tree,
    Chain,
    Force,
    Trie,
    Grid,
    Buckets,
    Queue,
    LinkedList,
    Forest,
    HeapDual,
}

impl RenderFamily {
    #[must_use]
    pub fn of(structure: VizStructure) -> Self {
        match structure {
            VizStructure::Array | VizStructure::Bitset | VizStructure::Fenwick => Self::Cells,
            VizStructure::Queue | VizStructure::Deque => Self::Queue,
            VizStructure::Stack | VizStructure::Callstack => Self::Stack,
            VizStructure::Tree | VizStructure::SegmentTree => Self::Tree,
            VizStructure::Heap => Self::HeapDual,
            VizStructure::List => Self::LinkedList,
            VizStructure::Skiplist => Self::Chain,
            VizStructure::Graph => Self::Force,
            VizStructure::Hashmap => Self::Buckets,
            VizStructure::UnionFind => Self::Forest,
            VizStructure::Trie => Self::Trie,
            VizStructure::Grid => Self::Grid,
        }
    }
}

#[cfg(test)]
mod tests {
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
}
