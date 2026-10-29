//! The URL split: only `{slug path}/_assets/_simulators/…` and public images are ever looked up.

#![allow(clippy::unwrap_used)]

use super::{Kind, split};

#[test]
fn a_lesson_path_and_its_file_split_at_the_assets_segment() {
    let (slugs, file, kind) = split("dsa/recursion/head/head/_assets/_simulators/index.html").unwrap();
    assert_eq!(slugs, ["dsa", "recursion", "head", "head"]);
    assert_eq!(file, "_simulators/index.html");
    assert_eq!(kind, Kind::Simulator);
}

#[test]
fn the_served_folder_itself_splits_so_it_can_redirect_to_its_slash_form() {
    let (_, file, _) = split("dsa/_assets/_simulators").unwrap();
    assert_eq!(file, "_simulators");
}

#[test]
fn only_the_simulators_subtree_and_public_images_are_served() {
    assert!(split("dsa/lesson/_assets/_diagrams/a.anim/base.d2").is_none());
    assert!(split("dsa/lesson/_assets/_simulatorsX/index.html").is_none());
    assert!(split("dsa/lesson/_assets/").is_none());
    assert!(
        split("dsa/lesson/_assets/c4.d2").is_none(),
        "a diagram source beside an image"
    );
    assert!(split("dsa/lesson/_assets/notes.md").is_none());
}

#[test]
fn an_image_beside_the_lesson_or_in_a_plain_subfolder_is_served() {
    for file in [
        "shot.png",
        "keycloak/login.PNG",
        "c4.svg",
        "a.webp",
        "b.jpg",
        "c.jpeg",
        "d.gif",
    ] {
        let rest = format!("book/lesson/_assets/{file}");
        let (_, served, kind) = split(&rest).unwrap();
        assert_eq!(served, file);
        assert_eq!(kind, Kind::Image);
    }
}

#[test]
fn an_image_inside_a_hidden_or_underscore_folder_is_not_served() {
    assert!(split("book/lesson/_assets/_drafts/shot.png").is_none());
    assert!(split("book/lesson/_assets/.cache/shot.png").is_none());
    assert!(split("book/lesson/_assets/../secret.png").is_none());
    assert!(split("book/lesson/_assets/a//shot.png").is_none());
    assert!(split("book/lesson/_assets/.png").is_none());
}

#[test]
fn a_path_without_an_assets_segment_is_not_an_asset() {
    assert!(split("dsa/lesson/index.html").is_none());
    assert!(
        split("_assets/_simulators/index.html").is_none(),
        "no slug path before it"
    );
}
