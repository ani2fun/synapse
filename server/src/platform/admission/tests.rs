//! The two limits, which one a refusal names, and that every ticket gives its place back.

#![allow(clippy::unwrap_used, clippy::panic)]

use super::*;

fn gate(per_caller: u32, total: u32) -> Arc<Admission> {
    Arc::new(Admission::new(per_caller, total))
}

#[test]
fn a_caller_at_their_limit_is_refused_as_busy() {
    let gate = gate(2, 10);
    let _a = gate.admit("anon:198.51.100.1").unwrap();
    let _b = gate.admit("anon:198.51.100.1").unwrap();
    assert_eq!(
        gate.admit("anon:198.51.100.1").err(),
        Some(Refused::CallerBusy { limit: 2 })
    );
    // Someone else is unaffected.
    assert!(gate.admit("anon:198.51.100.2").is_ok());
}

#[test]
fn a_full_queue_refuses_at_once_whoever_asks() {
    let gate = gate(2, 3);
    let _held: Vec<_> = ["a", "b", "c"].iter().map(|k| gate.admit(k).unwrap()).collect();
    assert_eq!(gate.admit("d").err(), Some(Refused::SandboxBusy));
}

#[test]
fn a_caller_who_is_the_reason_is_told_so_before_the_queue_is() {
    let gate = gate(1, 1);
    let _held = gate.admit("a").unwrap();
    assert_eq!(gate.admit("a").err(), Some(Refused::CallerBusy { limit: 1 }));
}

#[test]
fn dropping_a_ticket_gives_its_place_back() {
    let gate = gate(1, 1);
    let ticket = gate.admit("a").unwrap();
    assert_eq!(gate.in_flight(), 1);
    drop(ticket);
    assert_eq!(gate.in_flight(), 0);
    assert!(gate.admit("a").is_ok(), "the caller's own place came back too");
}

#[test]
fn a_ticket_dropped_by_a_panic_still_gives_its_place_back() {
    let gate = gate(1, 1);
    let inner = Arc::clone(&gate);
    let _ = std::panic::catch_unwind(move || {
        let _ticket = inner.admit("a").unwrap();
        panic!("the run blew up");
    });
    assert_eq!(gate.in_flight(), 0);
}

#[test]
fn zero_limits_floor_at_one_rather_than_refusing_everything() {
    let gate = gate(0, 0);
    assert!(gate.admit("a").is_ok());
}
