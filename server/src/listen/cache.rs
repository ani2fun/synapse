//! The audio cache: one MP3 per unit, filed under its [`cache_key`](super::cache_key).
//!
//! Best-effort by design. A unit that cannot be read is synthesised again and a unit that cannot
//! be written is simply not kept, so no cache failure ever reaches the reader — it only costs
//! engine time, and it is logged.
//!
//! Bounded by a periodic sweep rather than on every write: it walks the tree, and once the total
//! passes the ceiling it deletes the least recently USED files first. A hit refreshes the file's
//! mtime, so a lesson people keep coming back to stays while a one-off selection ages out.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::body::Bytes;

use crate::platform::blocking::run_blocking;

/// How often the sweeper checks the ceiling.
const SWEEP_EVERY: Duration = Duration::from_mins(10);

pub struct AudioCache {
    root: PathBuf,
    cap_bytes: u64,
}

/// What one sweep did, for its log line and for tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Swept {
    pub kept_bytes: u64,
    pub removed: usize,
}

impl AudioCache {
    pub fn new(root: impl Into<PathBuf>, cap_mib: u64) -> Self {
        Self {
            root: root.into(),
            cap_bytes: cap_mib.saturating_mul(1024 * 1024),
        }
    }

    /// Two-character fan-out, so no directory ever holds the whole cache.
    fn path(&self, key: &str) -> PathBuf {
        let shard = key.get(..2).unwrap_or("__");
        self.root.join(shard).join(format!("{key}.mp3"))
    }

    pub async fn get(&self, key: &str) -> Option<Bytes> {
        let path = self.path(key);
        run_blocking(move || {
            let bytes = std::fs::read(&path).ok()?;
            // Refresh the mtime: the sweep evicts by it, and a hit is a use.
            if let Ok(file) = std::fs::File::options().write(true).open(&path) {
                let _ = file.set_modified(SystemTime::now());
            }
            Some(Bytes::from(bytes))
        })
        .await
    }

    /// Written to a temporary name and renamed into place, so a reader never sees half a file.
    pub async fn put(&self, key: &str, bytes: Bytes) {
        let path = self.path(key);
        let written = run_blocking(move || -> std::io::Result<()> {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let partial = path.with_extension("partial");
            std::fs::write(&partial, &bytes)?;
            std::fs::rename(&partial, &path)
        })
        .await;
        if let Err(error) = written {
            tracing::warn!(%error, key, "listen cache: could not keep a unit");
        }
    }

    /// Delete the least recently used files until the cache is back under its ceiling.
    pub async fn sweep(&self) -> Swept {
        let root = self.root.clone();
        let cap = self.cap_bytes;
        run_blocking(move || sweep_dir(&root, cap)).await
    }
}

fn sweep_dir(root: &Path, cap: u64) -> Swept {
    let mut files: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
    for shard in std::fs::read_dir(root).into_iter().flatten().flatten() {
        for entry in std::fs::read_dir(shard.path()).into_iter().flatten().flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_file() {
                let used = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                files.push((used, meta.len(), entry.path()));
            }
        }
    }
    let mut kept: u64 = files.iter().map(|(_, len, _)| len).sum();
    let mut removed = 0;
    files.sort_by_key(|(used, _, _)| *used);
    for (_, len, path) in files {
        if kept <= cap {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            kept -= len;
            removed += 1;
        }
    }
    Swept {
        kept_bytes: kept,
        removed,
    }
}

/// Sweep on a fixed cadence for the life of the process.
pub fn spawn_sweeper(cache: Arc<AudioCache>) {
    tokio::spawn(async move {
        let mut every = tokio::time::interval(SWEEP_EVERY);
        loop {
            every.tick().await;
            let swept = cache.sweep().await;
            if swept.removed > 0 {
                tracing::info!(
                    removed = swept.removed,
                    kept_bytes = swept.kept_bytes,
                    "listen cache swept"
                );
            } else {
                tracing::debug!(kept_bytes = swept.kept_bytes, "listen cache under its ceiling");
            }
        }
    });
}
