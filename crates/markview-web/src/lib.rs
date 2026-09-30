//! The browser front end: Markview driven from JavaScript through WebAssembly.
//!
//! A browser is the only host this crate has. Every other target builds an
//! empty `rlib`, so the native workspace keeps compiling, testing and linting
//! without any browser code in it.
//!
//! The JavaScript contract these modules implement is frozen in
//! `docs/mvaac-web-demo.md`.

#[cfg(target_arch = "wasm32")]
mod api;
#[cfg(target_arch = "wasm32")]
mod fonts;
#[cfg(target_arch = "wasm32")]
mod selection;

#[cfg(target_arch = "wasm32")]
pub use api::{Markview, create};
