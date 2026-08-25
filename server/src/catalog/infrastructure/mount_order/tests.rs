use super::*;

fn root(id: &str) -> SourceRoot {
    SourceRoot::new(id, format!("/tmp/{id}"))
}

fn placement(id: &str) -> Placement {
    Placement {
        source_id: id.to_owned(),
        grouping: Vec::new(),
        order: None,
    }
}

/// The primary checkout has no placement of its own, so the two lists start uneven — and the
/// pinned prefixes have to be tracked separately or `pinned_only` would slice the wrong one.
fn booted() -> MountOrder {
    MountOrder::pinned(
        vec![root("synapse-content"), root("java-guide-local")],
        vec![placement("java-guide-local")],
    )
}

#[test]
fn the_primary_checkout_leads_and_pinned_order_survives() {
    let order = booted();
    let ids: Vec<&str> = order.roots().iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["synapse-content", "java-guide-local"]);
}

#[test]
fn a_registered_satellite_lands_behind_every_pinned_source() {
    let mut order = booted();
    order.append(root("dsa-guide"), placement("dsa-guide"));

    let ids: Vec<&str> = order.roots().iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        ids,
        ["synapse-content", "java-guide-local", "dsa-guide"],
        "append has no position argument — a satellite cannot reach the front"
    );
}

/// The regression this type exists for: while a book lives in BOTH the monorepo and its new
/// repository, the monorepo's copy must win the contested slug, and that is decided purely by
/// which source the merge reads first.
#[test]
fn the_primary_still_leads_after_many_registrations() {
    let mut order = booted();
    for id in ["dsa-guide", "python-guide", "system-design-guide"] {
        order.append(root(id), placement(id));
    }
    assert_eq!(
        order.roots().first().map(|r| r.id.as_str()),
        Some("synapse-content"),
        "first-source-wins means nothing if the primary can be displaced"
    );
}

#[test]
fn placements_follow_their_roots_in_the_same_order() {
    let mut order = booted();
    order.append(root("dsa-guide"), placement("dsa-guide"));

    let ids: Vec<&str> = order.placements().iter().map(|p| p.source_id.as_str()).collect();
    assert_eq!(ids, ["java-guide-local", "dsa-guide"]);
}

#[test]
fn a_reconcile_starts_over_from_the_booted_set() {
    let mut order = booted();
    order.append(root("dsa-guide"), placement("dsa-guide"));

    // What the sync loop does every tick: drop last tick's registered tail, keep the pinned set.
    let next = order.pinned_only();
    let ids: Vec<&str> = next.roots().iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["synapse-content", "java-guide-local"]);
    assert_eq!(
        next.placements().len(),
        1,
        "the pinned placement survives, the registered one does not"
    );
}

#[test]
fn pinned_only_is_itself_re_appendable() {
    // A tick appends onto the previous tick's pinned_only(), so the prefix must carry over.
    let mut order = booted().pinned_only();
    order.append(root("dsa-guide"), placement("dsa-guide"));
    assert_eq!(
        order.pinned_only().roots().len(),
        2,
        "pinned_only() must preserve the pinned length, not reset it to everything it holds"
    );
}
