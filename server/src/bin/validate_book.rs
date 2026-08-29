//! `validate-book <dir>` — does this content repository render the way its author thinks?
//!
//! It runs the SERVER'S OWN walker over the directory, so what it reports and what the site does
//! cannot drift. That matters more than it sounds: the authoring contract's rules are enforced in
//! code, not in prose, and until now the only way to discover you had broken one was to push and
//! look. A satellite guide repo maintained by someone else needs a gate that answers before the
//! push.
//!
//! Exit 0 on clean or warnings-only, 1 on any error — so CI can just run it.

use std::path::{Path, PathBuf};

use synapse_server::catalog::application::ContentRepository;
use synapse_server::catalog::domain::content_tree::BookMeta;
use synapse_server::catalog::domain::lint::{self, Severity, Sidecars};
use synapse_server::catalog::domain::merge::{self, Placement};
use synapse_server::catalog::domain::resolver;
use synapse_server::catalog::infrastructure::{FileSystemContentRepository, SourceRoot};

const SOURCE_ID: &str = "validate";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let dir = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    if !dir.is_dir() {
        eprintln!("✗ {} is not a directory", dir.display());
        std::process::exit(1);
    }

    let repo = FileSystemContentRepository::over(vec![SourceRoot::new(SOURCE_ID, &dir)], true);
    let sources = repo.load_sources().await?;
    let Some(source) = sources.into_iter().next() else {
        eprintln!("✗ nothing to walk under {}", dir.display());
        std::process::exit(1);
    };

    let shape = if source.book_meta.is_some() {
        "a book (its root carries book.json)"
    } else {
        "a collection (books live in subdirectories)"
    };
    println!("→ {} is {shape}", dir.display());

    let sidecars = scan_sidecars(&dir);
    let mut findings = lint::lint(&source, &sidecars);
    findings.extend(placement_findings(source.book_meta.as_ref()));

    // The walk itself is the structural authority: duplicate slugs, over-deep chapters and
    // non-slug paths are refusals, not warnings, and they stop the catalog building at all.
    match merge::assemble(std::slice::from_ref(&source), &[] as &[Placement]) {
        Ok(walk) => {
            report(&findings);
            for book in resolver::all_books(&walk.catalog) {
                let lessons = resolver::lessons_in_reading_order(book);
                println!(
                    "\n  {} — {} lesson(s), first: {}",
                    book.slug,
                    lessons.len(),
                    lessons.first().map_or("(none)", |(p, _)| p.as_str())
                );
                if lessons.is_empty() {
                    println!("    ✗ a book with no lessons renders as an empty shelf");
                }
            }
            let errors = findings.iter().filter(|f| f.severity == Severity::Error).count();
            if errors > 0 {
                eprintln!("\n✗ {errors} error(s)");
                std::process::exit(1);
            }
            println!("\n✓ renders");
        }
        Err(error) => {
            report(&findings);
            eprintln!("\n✗ the catalog will not build: {error}");
            std::process::exit(1);
        }
    }
    Ok(())
}

fn report(findings: &[lint::Finding]) {
    if findings.is_empty() {
        return;
    }
    println!();
    for finding in findings {
        let mark = match finding.severity {
            Severity::Error => "✗",
            Severity::Warning => "!",
        };
        println!("  {mark} {}: {}", finding.path, finding.message);
    }
}

/// The files the walker never loads but the server still reads.
fn scan_sidecars(root: &Path) -> Sidecars {
    let mut sidecars = Sidecars::default();
    collect(root, root, &mut sidecars);
    sidecars
}

fn collect(root: &Path, dir: &Path, out: &mut Sidecars) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, out);
        } else if let Ok(relative) = path.strip_prefix(root) {
            let relative = relative.to_string_lossy().replace('\\', "/");
            if name.ends_with(".tests.json") {
                out.test_suites.push(relative);
            }
        }
    }
}

/// A root-level `book.json` means the repository IS the book — a satellite — and a satellite is
/// positioned by its registration row, never by its own file. An `order` here is therefore read by
/// nobody: harmless today, misleading forever, because the next person to reorder the library will
/// find a number that looks authoritative and change it to no effect.
///
/// A warning, not an error: every satellite carries one right now, and this is what tells their
/// authors it can go.
fn placement_findings(book_meta: Option<&BookMeta>) -> Vec<lint::Finding> {
    let Some(meta) = book_meta else {
        // A collection: its books DO order via their own `book.json`, so nothing to say.
        return Vec::new();
    };
    match meta.order {
        None => Vec::new(),
        Some(order) => vec![lint::Finding {
            severity: Severity::Warning,
            path: "book.json".to_owned(),
            message: format!(
                "`order: {order}` is ignored here — a satellite's position comes from its \
                 registration row in /admin, so this field is read by nobody. Remove it, and set \
                 the order where it takes effect."
            ),
        }],
    }
}
