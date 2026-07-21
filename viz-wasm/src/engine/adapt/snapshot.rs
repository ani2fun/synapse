//! One step's heap with reachability + sentinel logic (oracle: `HeapSnapshot.scala`).
//! Segmentation, rooting, and projection all read the SAME memoized, deterministic view.
//! Reachability is ITERATIVE (explicit stack, reverse-pushed children) — no stack-overflow
//! risk on deep structures — with a preorder IDENTICAL to a recursive DFS.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::engine::adapt::vocab;
use crate::engine::trace::{HeapObject, HeapScalar, HeapValue};

pub struct HeapSnapshot<'a> {
    pub heap: &'a BTreeMap<String, HeapObject>,
    memo: RefCell<HashMap<String, Vec<String>>>,
}

impl<'a> HeapSnapshot<'a> {
    pub fn new(heap: &'a BTreeMap<String, HeapObject>) -> Self {
        Self {
            heap,
            memo: RefCell::new(HashMap::new()),
        }
    }

    /// The out-references of a heap object, in field/element order (Dict: key then value).
    #[must_use]
    pub fn out_refs(obj: &HeapObject) -> Vec<&str> {
        match obj {
            HeapObject::Instance { fields, .. } => fields
                .iter()
                .filter_map(|(_, v)| match v {
                    HeapValue::Ref(to) => Some(to.as_str()),
                    HeapValue::Scalar(_) => None,
                })
                .collect(),
            HeapObject::Arr { items, .. } => items
                .iter()
                .filter_map(|v| match v {
                    HeapValue::Ref(to) => Some(to.as_str()),
                    HeapValue::Scalar(_) => None,
                })
                .collect(),
            HeapObject::Dict { entries } => entries
                .iter()
                .flat_map(|(k, v)| [k, v])
                .filter_map(|v| match v {
                    HeapValue::Ref(to) => Some(to.as_str()),
                    HeapValue::Scalar(_) => None,
                })
                .collect(),
        }
    }

    /// Every id reachable from `start`, in preorder DFS. Memoized per start.
    #[must_use]
    pub fn reachable_from(&self, start: &str) -> Vec<String> {
        if let Some(cached) = self.memo.borrow().get(start) {
            return cached.clone();
        }
        let mut seen: Vec<String> = Vec::new();
        let mut seen_set: HashSet<String> = HashSet::new();
        let mut stack = vec![start.to_owned()];
        while let Some(id) = stack.pop() {
            if let Some(obj) = self.heap.get(&id)
                && seen_set.insert(id.clone())
            {
                seen.push(id);
                for to in Self::out_refs(obj).into_iter().rev() {
                    stack.push(to.to_owned());
                }
            }
        }
        self.memo.borrow_mut().insert(start.to_owned(), seen.clone());
        seen
    }

    /// A CLRS NIL sentinel: an instance whose primary value field (`VALUE_FIELDS` order) is
    /// null AND whose every reference field is a self-loop. Elided from the drawn graph.
    #[must_use]
    pub fn is_null_sentinel(&self, id: &str) -> bool {
        match self.heap.get(id) {
            Some(HeapObject::Instance { fields, .. }) => {
                let primary_null = vocab::VALUE_FIELDS
                    .iter()
                    .find_map(|vf| fields.iter().find(|(n, _)| n == vf))
                    .is_some_and(|(_, v)| matches!(v, HeapValue::Scalar(HeapScalar::Null)));
                let refs_all_self = fields.iter().all(|(_, v)| match v {
                    HeapValue::Ref(to) => to == id,
                    HeapValue::Scalar(_) => true,
                });
                primary_null && refs_all_self
            }
            _ => false,
        }
    }

    /// Heap ids in sorted order — a deterministic scan.
    #[must_use]
    pub fn ids_sorted(&self) -> Vec<&str> {
        self.heap.keys().map(String::as_str).collect() // BTreeMap keys are already sorted
    }
}
