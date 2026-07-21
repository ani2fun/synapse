//! FFI externs the viz surfaces need — in-crate copies of the client's `islands/` bindings
//! (A10): the crate must resolve `@editor/loader` / `@tracer/loader` through WHICHEVER Vite
//! build bundles its wasm-bindgen glue (the old client's or the Astro app's), so the extern
//! declarations travel with the crate instead of reaching back into a host crate.

pub mod editor;
pub mod tracer;
