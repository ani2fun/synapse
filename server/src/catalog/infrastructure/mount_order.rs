//! The mount list, in merge order.
//!
//! First-source-wins settles a contested book slug (`domain::merge`), so the ORDER of the mounted
//! sources is what decides which copy of a migrating book serves — and the answer has to be the
//! copy that was already there, every time, or deletion day moves live URLs.
//!
//! That rule used to hold by agreement between three places that never mention each other: the
//! order `main` happened to push roots in at boot, the order the sync loop happened to rebuild
//! them in on each tick, and the registry query's `order by`. Each read innocently on its own, and
//! inverting any one of them would have flipped a migration-window winner with every test green.
//!
//! Here it is structural instead. Pinned sources can only be supplied at construction, registered
//! ones can only be appended, and nothing hands out a list a caller could reorder — so there is no
//! way to put a satellite ahead of the primary checkout.

use crate::catalog::domain::merge::Placement;
use crate::catalog::infrastructure::filesystem::SourceRoot;

/// Mounted sources in the order the merge reads them: pinned first, registered after.
///
/// Roots and placements are parallel only among the registered tail. The primary checkout has a
/// root and no placement — it IS the library's top level rather than something grafted into it —
/// so the two lists carry their own pinned prefix lengths.
#[derive(Clone, Default)]
pub struct MountOrder {
    roots: Vec<SourceRoot>,
    placements: Vec<Placement>,
    pinned_roots: usize,
    pinned_placements: usize,
}

impl MountOrder {
    /// The sources mounted regardless of the registry: the git-sync'd primary checkout FIRST,
    /// then anything mounted from local disk. Neither is a registry row, so a reconcile rebuilt
    /// from the registry alone would drop them — every tick starts from this set.
    ///
    /// The caller supplies the pinned order because only it knows what it mounted; what this type
    /// guarantees is that nothing later displaces it.
    #[must_use]
    pub fn pinned(roots: Vec<SourceRoot>, placements: Vec<Placement>) -> Self {
        Self {
            pinned_roots: roots.len(),
            pinned_placements: placements.len(),
            roots,
            placements,
        }
    }

    /// Append a registered satellite. It lands after every pinned source — there is no other
    /// position available.
    pub fn append(&mut self, root: SourceRoot, placement: Placement) {
        self.roots.push(root);
        self.placements.push(placement);
    }

    /// The pinned prefix alone, for a reconcile starting over from what the process booted with.
    #[must_use]
    pub fn pinned_only(&self) -> Self {
        Self::pinned(
            self.roots[..self.pinned_roots].to_vec(),
            self.placements[..self.pinned_placements].to_vec(),
        )
    }

    #[must_use]
    pub fn roots(&self) -> &[SourceRoot] {
        &self.roots
    }

    #[must_use]
    pub fn placements(&self) -> &[Placement] {
        &self.placements
    }

    /// Consume into the two lists the publishers take. Ordering is already decided by now.
    #[must_use]
    pub fn into_parts(self) -> (Vec<SourceRoot>, Vec<Placement>) {
        (self.roots, self.placements)
    }
}

#[cfg(test)]
mod tests;
