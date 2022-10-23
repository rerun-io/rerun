//! An Instrumental Memory Profiler for Rust.
//!
//! ## Example:
//!
//! ```
//!
//! ```

mod text_tree;
mod tracking_allocator;
mod trait_impls;
mod types;

pub use {text_tree::TextTree, tracking_allocator::TrackingAllocator, types::*};
