//! Untrusted-archive handling. One theme: nothing an archive says may put a byte outside the
//! commit directory, and a reader following `current` never sees a half-written tree.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;

use super::*;
use crate::catalog::infrastructure::commit_sha::read_commit_sha;

fn gzip(tar: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(tar).unwrap();
    encoder.finish().unwrap()
}

/// Build a gzipped tar the way GitHub does: everything under one wrapping directory.
fn archive(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (path, body) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, path, body.as_bytes()).unwrap();
    }
    gzip(&builder.into_inner().unwrap())
}

/// A HOSTILE archive. `Builder::append_data` refuses to write `..` or an absolute path — which is
/// itself the point: a well-behaved writer cannot produce these, so the only way one reaches us is
/// deliberately, and the only way to test the guard is to write the name bytes by hand.
fn archive_with_raw_name(name: &str, body: &str) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());

    let mut wrapper = tar::Header::new_gnu();
    wrapper.set_size(2);
    wrapper.set_mode(0o644);
    wrapper.set_cksum();
    builder
        .append_data(&mut wrapper, "wrap/book.json", &b"{}"[..])
        .unwrap();

    let mut hostile = tar::Header::new_gnu();
    hostile.set_size(body.len() as u64);
    hostile.set_mode(0o644);
    hostile.set_entry_type(tar::EntryType::Regular);
    {
        let gnu = hostile.as_gnu_mut().expect("a gnu header");
        let bytes = name.as_bytes();
        gnu.name[..bytes.len()].copy_from_slice(bytes);
    }
    hostile.set_cksum();
    builder.append(&hostile, body.as_bytes()).unwrap();

    gzip(&builder.into_inner().unwrap())
}

#[test]
fn the_wrapping_directory_is_stripped_and_the_book_lands_at_the_root() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = ContentCache::new(tmp.path());
    let bytes = archive(&[
        ("ani2fun-java-guide-abc1234/book.json", r#"{"slug":"java"}"#),
        ("ani2fun-java-guide-abc1234/01-first-steps/01-intro.md", "# Intro"),
    ]);

    let checkout = cache.publish("java-guide", "abc1234", &bytes).unwrap();

    assert_eq!(checkout, cache.checkout_of("java-guide"));
    assert_eq!(
        std::fs::read_to_string(checkout.join("book.json")).unwrap(),
        r#"{"slug":"java"}"#
    );
    assert_eq!(
        std::fs::read_to_string(checkout.join("01-first-steps/01-intro.md")).unwrap(),
        "# Intro"
    );
}

#[test]
fn an_entry_that_escapes_its_root_is_refused_and_nothing_is_written() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("outside.txt");
    let cache = ContentCache::new(tmp.path().join("cache"));
    // After the wrapping component is stripped this is `../../outside.txt`, relative to the
    // commit directory — i.e. straight out of the cache.
    let bytes = archive_with_raw_name("wrap/../../../outside.txt", "escaped!");

    let error = cache.publish("evil", "sha1", &bytes).unwrap_err();

    assert!(matches!(error, FetchError::Malformed(_)), "{error:?}");
    assert!(!outside.exists(), "the traversal must not have written");
    // And the partial tree is gone, so `current` can never be pointed at it.
    assert!(!cache.checkout_of("evil").exists());
}

#[test]
fn an_absolute_entry_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = ContentCache::new(tmp.path());
    let planted = tmp.path().join("planted.txt");
    let bytes = archive_with_raw_name(&planted.display().to_string(), "nope");

    let error = cache.publish("abs", "sha1", &bytes).unwrap_err();

    assert!(matches!(error, FetchError::Malformed(_)), "{error:?}");
    assert!(!planted.exists(), "an absolute entry must not have written");
}

#[test]
fn a_second_commit_flips_current_and_prunes_the_first() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = ContentCache::new(tmp.path());

    cache
        .publish("java-guide", "sha-one", &archive(&[("w/a.md", "first")]))
        .unwrap();
    cache
        .publish("java-guide", "sha-two", &archive(&[("w/a.md", "second")]))
        .unwrap();

    let checkout = cache.checkout_of("java-guide");
    assert_eq!(std::fs::read_to_string(checkout.join("a.md")).unwrap(), "second");
    assert!(
        !tmp.path().join("java-guide/sha-one").exists(),
        "the superseded commit is reclaimed"
    );
    assert!(tmp.path().join("java-guide/sha-two").exists());
}

#[test]
fn symlink_entries_are_dropped_rather_than_followed() {
    // A symlink pointing outside the tree turns a later ordinary write into an escape.
    let tmp = tempfile::tempdir().unwrap();
    let cache = ContentCache::new(tmp.path());

    let mut builder = tar::Builder::new(Vec::new());
    let mut link = tar::Header::new_gnu();
    link.set_entry_type(tar::EntryType::Symlink);
    link.set_size(0);
    link.set_mode(0o777);
    builder.append_link(&mut link, "wrap/escape", "/tmp").unwrap();
    let mut file = tar::Header::new_gnu();
    file.set_size(4);
    file.set_mode(0o644);
    file.set_cksum();
    builder
        .append_data(&mut file, "wrap/ok.md", &b"fine"[..])
        .unwrap();
    let tar = builder.into_inner().unwrap();
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&tar).unwrap();
    let bytes = encoder.finish().unwrap();

    let checkout = cache.publish("linky", "sha1", &bytes).unwrap();
    assert!(checkout.join("ok.md").exists());
    assert!(
        !checkout.join("escape").exists(),
        "the symlink must not have been created"
    );
}

#[test]
fn garbage_is_refused_not_unpacked() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = ContentCache::new(tmp.path());
    let error = cache.publish("junk", "sha1", b"not a gzip stream").unwrap_err();
    assert!(matches!(error, FetchError::Malformed(_)), "{error:?}");
    assert!(!cache.checkout_of("junk").exists());
}

#[test]
fn forgetting_a_source_reclaims_its_whole_cache() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = ContentCache::new(tmp.path());
    cache
        .publish("gone", "sha1", &archive(&[("w/a.md", "x")]))
        .unwrap();
    assert!(tmp.path().join("gone").exists());

    cache.forget("gone");
    assert!(!tmp.path().join("gone").exists());
}

/// `checkout_of` is a STABLE path — `current` never changes name — so the mounted root cannot tell
/// the catalog that anything moved. The content version has to, and for a tarball checkout the only
/// commit id on disk is the directory `current` resolves to. Without this the version is identical
/// before and after a publish, the version-gated index never rebuilds, and a satellite serves the
/// commit it booted with no matter what lands.
#[test]
fn publishing_a_new_commit_moves_the_content_version() {
    const FIRST: &str = "4ecf01b5a5acc83dd2b64ebd2c9f054de868aa53";
    const SECOND: &str = "d2dad749889b3dd4471356da8e47fba9fae42e21";

    let tmp = tempfile::tempdir().unwrap();
    let cache = ContentCache::new(tmp.path());

    cache
        .publish("dsa-guide", FIRST, &archive(&[("w/book.json", "{}")]))
        .unwrap();
    let before = read_commit_sha(&cache.checkout_of("dsa-guide"));

    cache
        .publish(
            "dsa-guide",
            SECOND,
            &archive(&[("w/book.json", "{}"), ("w/new-lesson.md", "# New")]),
        )
        .unwrap();
    let after = read_commit_sha(&cache.checkout_of("dsa-guide"));

    assert_eq!(before, FIRST);
    assert_eq!(after, SECOND);
    assert_ne!(
        before, after,
        "a landed commit must move the version, or the catalog cache never rebuilds"
    );
}
