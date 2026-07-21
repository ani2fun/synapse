//! The one authored structure vocabulary: ONE attribute, `viz=<structure>[:<root>]`.
//! An unknown token has no entry → the caller shows an honest error card, never a silent guess.

/// The geometry family a structure lays out with — WHERE its nodes sit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutKind {
    Cells,
    Grid,
    Tree,
    Chain,
    Graph,
}

/// The closed set of authored data-structure vocabularies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VizStructure {
    Array,
    Grid,
    Stack,
    Queue,
    Deque,
    Tree,
    Heap,
    List,
    Hashmap,
    Graph,
    Trie,
    UnionFind,
    Fenwick,
    Bitset,
    Skiplist,
    SegmentTree,
    Callstack,
}

impl VizStructure {
    pub const ALL: [Self; 17] = [
        Self::Array,
        Self::Grid,
        Self::Stack,
        Self::Queue,
        Self::Deque,
        Self::Tree,
        Self::Heap,
        Self::List,
        Self::Hashmap,
        Self::Graph,
        Self::Trie,
        Self::UnionFind,
        Self::Fenwick,
        Self::Bitset,
        Self::Skiplist,
        Self::SegmentTree,
        Self::Callstack,
    ];

    /// The structure for a kebab-case token, if it's in the vocabulary.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_lowercase().as_str() {
            "array" => Some(Self::Array),
            "grid" => Some(Self::Grid),
            "stack" => Some(Self::Stack),
            "queue" => Some(Self::Queue),
            "deque" => Some(Self::Deque),
            "tree" => Some(Self::Tree),
            "heap" => Some(Self::Heap),
            "list" => Some(Self::List),
            "hashmap" => Some(Self::Hashmap),
            "graph" => Some(Self::Graph),
            "trie" => Some(Self::Trie),
            "union-find" => Some(Self::UnionFind),
            "fenwick" => Some(Self::Fenwick),
            "bitset" => Some(Self::Bitset),
            "skiplist" => Some(Self::Skiplist),
            "segment-tree" => Some(Self::SegmentTree),
            "callstack" => Some(Self::Callstack),
            _ => None,
        }
    }

    /// Parse an authored `viz=` value: `<structure>[:<root>]` → the structure + an optional
    /// root variable (which may be dotted, e.g. `list:self.head`). `None` on an unknown name.
    #[must_use]
    pub fn parse(token: &str) -> Option<(Self, Option<String>)> {
        let t = token.trim();
        let (name, root) = match t.find(':') {
            None => (t, None),
            Some(colon) => {
                let root = t[colon + 1..].trim();
                (&t[..colon], Some(root.to_owned()).filter(|r| !r.is_empty()))
            }
        };
        Self::from_name(name).map(|s| (s, root))
    }

    /// The geometry family this structure lays out with.
    #[must_use]
    pub fn layout(self) -> LayoutKind {
        match self {
            Self::Array
            | Self::Stack
            | Self::Queue
            | Self::Deque
            | Self::Bitset
            | Self::Fenwick
            | Self::Skiplist
            | Self::Callstack => LayoutKind::Cells,
            Self::Grid => LayoutKind::Grid,
            Self::Tree | Self::Heap | Self::SegmentTree | Self::Trie => LayoutKind::Tree,
            Self::List => LayoutKind::Chain,
            Self::Graph | Self::UnionFind | Self::Hashmap => LayoutKind::Graph,
        }
    }

    /// The canonical authored token (kebab-case).
    #[must_use]
    pub fn token(self) -> &'static str {
        match self {
            Self::Array => "array",
            Self::Grid => "grid",
            Self::Stack => "stack",
            Self::Queue => "queue",
            Self::Deque => "deque",
            Self::Tree => "tree",
            Self::Heap => "heap",
            Self::List => "list",
            Self::Hashmap => "hashmap",
            Self::Graph => "graph",
            Self::Trie => "trie",
            Self::UnionFind => "union-find",
            Self::Fenwick => "fenwick",
            Self::Bitset => "bitset",
            Self::Skiplist => "skiplist",
            Self::SegmentTree => "segment-tree",
            Self::Callstack => "callstack",
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
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
}
