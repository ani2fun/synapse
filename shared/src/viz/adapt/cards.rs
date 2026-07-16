//! Union-find over instance→instance refs (oracle: `CardGrouping.scala`). Instances linked by
//! reference merge into one card (a tree, a linked list); collections each own their own
//! card. The representative is the lexicographically-smallest id, so grouping is
//! deterministic. The `contains` guard drops refs to elided nulls.

use std::collections::{BTreeMap, HashMap};

use crate::viz::trace::{HeapObject, HeapValue};

fn find(parent: &mut HashMap<String, String>, id: &str) -> String {
    let mut root = id.to_owned();
    while parent[&root] != root {
        root.clone_from(&parent[&root]);
    }
    let mut cur = id.to_owned();
    while parent[&cur] != root {
        let next = parent[&cur].clone();
        parent.insert(cur, root.clone());
        cur = next;
    }
    root
}

fn union(parent: &mut HashMap<String, String>, a: &str, b: &str) {
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra != rb {
        if ra < rb {
            parent.insert(rb, ra);
        } else {
            parent.insert(ra, rb);
        }
    }
}

/// `obj_id → card_id` over the reachable set.
#[must_use]
pub fn group_cards(reachable: &[String], heap: &BTreeMap<String, HeapObject>) -> HashMap<String, String> {
    let mut parent: HashMap<String, String> = reachable.iter().map(|id| (id.clone(), id.clone())).collect();

    for id in reachable {
        if let Some(HeapObject::Instance { fields, .. }) = heap.get(id) {
            for (_, v) in fields {
                if let HeapValue::Ref(to_id) = v
                    && parent.contains_key(to_id)
                    && matches!(heap.get(to_id), Some(HeapObject::Instance { .. }))
                {
                    union(&mut parent, id, to_id);
                }
            }
        }
    }

    reachable
        .iter()
        .map(|id| (id.clone(), find(&mut parent, id)))
        .collect()
}
