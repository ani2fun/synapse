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
    findings.extend(c4_findings(&dir, source.book_meta.is_some(), &sidecars));

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
            // Case-sensitive on purpose, like the walker's own `.md` check: content extensions
            // are lowercase by convention, and `.C4` should not silently count.
            if name.ends_with(".tests.json") {
                out.test_suites.push(relative);
            } else if std::path::Path::new(&name).extension().is_some_and(|e| e == "c4") {
                out.c4_files.push(relative);
            }
        }
    }
}

/// The C4 rules a satellite has to keep. The merged `/c4` workspace carries exactly ONE
/// `specification {}`, and it lives in the spine — a satellite that ships its own makes the whole
/// workspace ambiguous, and the failure is a blank iframe rather than an error.
fn c4_findings(root: &Path, is_book_source: bool, sidecars: &Sidecars) -> Vec<lint::Finding> {
    let mut findings = Vec::new();
    if !is_book_source {
        return findings;
    }
    for file in &sidecars.c4_files {
        let Ok(body) = std::fs::read_to_string(root.join(file)) else {
            continue;
        };
        if body
            .lines()
            .any(|line| line.trim_start().starts_with("specification") && line.contains('{'))
        {
            findings.push(lint::Finding {
                severity: Severity::Error,
                path: file.clone(),
                message: "a satellite must not declare `specification {}` — the merged /c4 \
                          workspace has exactly one, in the spine repository"
                    .to_owned(),
            });
        }
        findings.extend(view_prefix_findings(file, &body));
    }
    findings
}

/// View ids share ONE global namespace across the merged workspace, so a satellite's must carry a
/// prefix nobody else uses. A collision does not error — it silently resolves to whichever view
/// the build saw last, and the wrong diagram appears in someone else's book.
fn view_prefix_findings(file: &str, body: &str) -> Vec<lint::Finding> {
    let ids: Vec<&str> = body
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("view "))
        .filter_map(|rest| rest.split_whitespace().next())
        .filter(|id| *id != "{")
        .collect();
    let unprefixed: Vec<&&str> = ids.iter().filter(|id| !id.contains('_')).collect();
    if unprefixed.is_empty() {
        return Vec::new();
    }
    vec![lint::Finding {
        severity: Severity::Warning,
        path: file.to_owned(),
        message: format!(
            "view id(s) with no `<prefix>_` segment ({}) — ids are global across the merged /c4 \
             workspace, so an unprefixed one can collide with another repository's",
            unprefixed
                .iter()
                .map(|id| (**id).to_owned())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }]
}
