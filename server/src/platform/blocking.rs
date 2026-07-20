//! The one `run_blocking` (step 62) — formerly a byte-identical twin in the catalog and
//! blog filesystem adapters. Infrastructure-side by nature (tokio), so it lives in
//! `platform`, never under a `domain/`.

/// Run blocking filesystem work off the async workers.
pub(crate) async fn run_blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    match tokio::task::spawn_blocking(work).await {
        Ok(value) => value,
        // A panicked blocking task is a bug upstream; surfacing it as a panic here would just
        // hide the original. Propagate by resuming the unwind.
        Err(join_error) => std::panic::resume_unwind(join_error.into_panic()),
    }
}
