//! The browser front end: Markview driven from JavaScript through WebAssembly.
//!
//! A browser is the only host this crate has. Every other target builds an
//! empty `rlib`, so the native workspace keeps compiling, testing and linting
//! without any browser code in it.
//!
//! The JavaScript contract these modules implement is frozen in
//! `docs/mvaac-web-demo.md`.

// The pointer and the publication bookkeeping are pure state: they name no
// browser type, so native tests cover them directly and `api.rs` stays the
// only module a browser has to host. Only the wasm front end constructs them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod selection;
mod state;

#[cfg(target_arch = "wasm32")]
mod api;
#[cfg(target_arch = "wasm32")]
mod fonts;

#[cfg(target_arch = "wasm32")]
pub use api::{Markview, create};
