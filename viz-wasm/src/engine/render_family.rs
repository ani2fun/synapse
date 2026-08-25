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
mod tests;
