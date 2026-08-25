//! Content types for simulator bundles. `nosniff` is on, so a wrong type is a blank frame:
//! every served extension is declared, and an unknown one downloads rather than guesses.

#![allow(clippy::unwrap_used)]

use std::path::Path;

use super::content_type_of;

#[test]
fn bundle_types_are_explicit_because_nosniff_refuses_wrong_ones() {
    assert_eq!(
        content_type_of(Path::new("a/index.html")),
        "text/html; charset=utf-8"
    );
    assert_eq!(content_type_of(Path::new("a/app.js")), "text/javascript");
    assert_eq!(content_type_of(Path::new("a/app.mjs")), "text/javascript");
    assert_eq!(content_type_of(Path::new("a/app.css")), "text/css; charset=utf-8");
    assert_eq!(content_type_of(Path::new("a/app.wasm")), "application/wasm");
    assert_eq!(content_type_of(Path::new("a/font.woff2")), "font/woff2");
}

#[test]
fn the_unknown_extension_downloads_rather_than_guesses() {
    assert_eq!(
        content_type_of(Path::new("a/tool.exe")),
        "application/octet-stream"
    );
    assert_eq!(
        content_type_of(Path::new("a/no-extension")),
        "application/octet-stream"
    );
}
