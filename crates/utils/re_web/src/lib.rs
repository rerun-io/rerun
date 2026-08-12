//! Utilities for interacting with Web APIs.

pub mod browser;

#[cfg(target_arch = "wasm32")]
mod error;
#[cfg(target_arch = "wasm32")]
pub mod fs;

#[cfg(target_arch = "wasm32")]
pub use error::Error;
