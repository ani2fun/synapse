//! What a walkthrough's sidecar directory is allowed to serve: `.svg` and `boards.json`, each
//! declaring an explicit content type. Traversal is deliberately NOT rejected here — the slug
//! check owns that, and duplicating it would hide which layer holds the line.

use super::*;
use crate::catalog::domain::walker;

#[test]
fn reads_a_board_and_its_manifest() {
    assert_eq!(
        BoardFile::parse("container.svg"),
        Some(("container", BoardFile::Svg))
    );
    assert_eq!(BoardFile::parse("root.svg"), Some(("root", BoardFile::Svg)));
    assert_eq!(
        BoardFile::parse("boards.json"),
        Some(("boards", BoardFile::Manifest))
    );
}

#[test]
fn refuses_every_other_extension() {
    for name in [
        "container",
        "container.png",
        "notes.json",
        "",
        "boards.json.svg.txt",
    ] {
        assert_eq!(BoardFile::parse(name), None, "{name} should not be served");
    }
}

#[test]
fn leaves_a_traversal_for_the_slug_check_to_reject() {
    // This layer decides the EXTENSION only. A traversal still parses here and is stopped by
    // the caller's `slug_like` on the stem — asserted so the division of labour is explicit
    // and neither side can be dropped on the assumption that the other covers it.
    assert_eq!(
        BoardFile::parse("../../secret.svg"),
        Some(("../../secret", BoardFile::Svg))
    );
    assert_eq!(BoardFile::parse(".svg"), Some(("", BoardFile::Svg)));
    for (stem, _) in [BoardFile::parse("../../secret.svg"), BoardFile::parse(".svg")]
        .into_iter()
        .flatten()
    {
        assert!(!walker::slug_like(stem), "{stem} must fail the slug check");
    }
}

#[test]
fn each_kind_declares_its_content_type() {
    assert_eq!(BoardFile::Svg.content_type(), "image/svg+xml");
    assert_eq!(BoardFile::Manifest.content_type(), "application/json");
}
