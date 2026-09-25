//! The URL split: only `{slug path}/_assets/_simulators/…` is ever looked up.

#![allow(clippy::unwrap_used)]

use super::split;

#[test]
fn a_lesson_path_and_its_file_split_at_the_assets_segment() {
    let (slugs, file) = split("dsa/recursion/head/head/_assets/_simulators/index.html").unwrap();
    assert_eq!(slugs, ["dsa", "recursion", "head", "head"]);
    assert_eq!(file, "_simulators/index.html");
}

#[test]
fn the_served_folder_itself_splits_so_it_can_redirect_to_its_slash_form() {
    let (_, file) = split("dsa/_assets/_simulators").unwrap();
    assert_eq!(file, "_simulators");
}

#[test]
fn only_the_simulators_subtree_is_served() {
    assert!(split("dsa/lesson/_assets/_diagrams/a.anim/base.d2").is_none());
    assert!(split("dsa/lesson/_assets/_simulatorsX/index.html").is_none());
    assert!(split("dsa/lesson/_assets/").is_none());
}

#[test]
fn a_path_without_an_assets_segment_is_not_an_asset() {
    assert!(split("dsa/lesson/index.html").is_none());
    assert!(
        split("_assets/_simulators/index.html").is_none(),
        "no slug path before it"
    );
}
