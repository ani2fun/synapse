//! Which `_assets/` files are published, and which image references a lesson makes.

use super::*;

#[test]
fn an_image_beside_the_lesson_or_in_a_plain_subfolder_is_public() {
    for file in [
        "shot.png",
        "keycloak/login.PNG",
        "c4.svg",
        "a.webp",
        "b.jpg",
        "c.jpeg",
        "d.gif",
    ] {
        assert!(is_public_image(file), "{file}");
    }
}

#[test]
fn hidden_folders_sources_and_traversal_are_not_public() {
    for file in [
        "_drafts/a.png",
        ".cache/a.png",
        "../a.png",
        "a//b.png",
        ".png",
        "c4.d2",
        "notes.md",
    ] {
        assert!(!is_public_image(file), "{file}");
    }
}

#[test]
fn local_refs_are_read_from_prose_but_not_from_code() {
    let md = "![a](_assets/a.png) and ![b](./_assets/s/b.svg \"title\")\n\
              ![c](/media/java/c.png) ![d](https://x/_assets/d.png)\n\
              ```md\n![e](_assets/e.png)\n```\n";
    assert_eq!(local_image_refs(md), ["a.png", "s/b.svg"]);
}
