//! Which colour a pointer name earns. Known roles keep their canonical colour and aliases resolve
//! to the same one; everything else is assigned by first appearance, so a trace that names the
//! same pointer twice does not spend two colours on it.

#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn known_pointer_names_get_their_canonical_role_colour() {
    assert_eq!(role_color("head"), Some("#3a5a8c"));
    assert_eq!(role_color("current"), Some("#4f5bd5"));
    assert_eq!(role_color("tail"), Some("#a13e3e"));
}

#[test]
fn aliases_resolve_to_their_canonical_colour() {
    assert_eq!(role_color("cur"), canon("current"));
    assert_eq!(role_color("prev"), canon("previous"));
    assert_eq!(role_color("lo"), canon("low"));
}

#[test]
fn an_unknown_name_has_no_role_colour() {
    assert_eq!(role_color("zzz"), None);
}

#[test]
fn assign_colors_gives_known_roles_their_colour_distinctly() {
    let m = assign_colors(&["i".into(), "j".into(), "cur".into()]);
    assert_eq!(m["i"], "#3a5a8c");
    assert_eq!(m["j"], "#8a4f7d");
    assert_eq!(m["cur"], "#4f5bd5");
}

#[test]
fn assign_colors_falls_back_by_first_appearance() {
    let m = assign_colors(&["aa".into(), "bb".into()]);
    assert_eq!(m["aa"], FALLBACK[0]);
    assert_eq!(m["bb"], FALLBACK[1]);
}

#[test]
fn assign_colors_dedups_repeated_names() {
    let m = assign_colors(&["x".into(), "x".into(), "y".into()]);
    assert_eq!(m.len(), 2);
    assert_eq!(m["x"], FALLBACK[0]);
    assert_eq!(m["y"], FALLBACK[1]);
}
