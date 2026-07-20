//! Pure execution logic (the logic layer — no leptos, no web-sys; purity-gated,
//! native-tested).

mod blocks;
mod executor;
mod language;
mod practice;

pub use blocks::{
    Variant, can_reproduce, can_visualise, display_lang, expected_for, parse_variants, seed_values,
};
pub use executor::{EditMode, ExecutorState, RunHandle, RunState, changed_line_count, is_dirty};
pub use language::{canonical_lang, preferred_index};
pub use practice::{Approach, PracticeSpec, decode_practice, solution_complexities};
