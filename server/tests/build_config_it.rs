//! The build-configuration lock.
//!
//! Not a behaviour the server needs — a guarantee that the binary CI tests is the binary the
//! Dockerfile ships.
//!
//! Cargo unifies features across whatever is built together. `cargo test --workspace` builds
//! the client too, and the client declares `serde_json/preserve_order` (its viz decoder needs
//! locals in insertion order). `cargo build --release -p synapse-server` — what Dockerfile:34
//! runs — builds the server alone and got no such feature. So `serde_json::Value` was backed
//! by `IndexMap` in CI and `BTreeMap` in production:
//!
//!     CI      serde_json  default,indexmap,preserve_order,raw_value,std
//!     Docker  serde_json  default,raw_value,std
//!
//! Nothing was actually broken by it. Every server-side `Value` site is order-insensitive:
//! the Ollama and go-judge request bodies, `outcome jsonb` (Postgres normalises key order),
//! and `contract_it` compares through a `BTreeSet`. But it is the worst SHAPE of latent bug —
//! the next order-sensitive line would have been green in CI and wrong in production, with no
//! signal pointing at the build configuration.
//!
//! The fix was to declare the feature on the server too, so every invocation produces the
//! same `serde_json`. This test is what stops that drifting back: it asserts the BEHAVIOUR the
//! feature provides, so deleting the line from `server/Cargo.toml` fails here rather than
//! silently re-opening the gap between test and ship.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

/// Keys deliberately out of alphabetical order. With `preserve_order` they round-trip in
/// INSERTION order; without it `Value`'s `BTreeMap` sorts them and this fails.
#[test]
fn serde_json_preserves_object_key_order_in_the_server_build() {
    let raw = r#"{"zebra":1,"alpha":2,"middle":3}"#;
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let round_tripped = serde_json::to_string(&value).unwrap();

    assert_eq!(
        round_tripped, raw,
        "serde_json/preserve_order is missing from the SERVER build — CI and the Dockerfile \
         are compiling different binaries again. Restore `features = [\"preserve_order\"]` on \
         serde_json in server/Cargo.toml."
    );

    let object = value.as_object().unwrap();
    let keys: Vec<&str> = object.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        ["zebra", "alpha", "middle"],
        "object keys must iterate in insertion order, not sorted order"
    );
}
