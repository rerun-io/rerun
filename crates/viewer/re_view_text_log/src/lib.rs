//! Rerun `TextLog` View
//!
//! A View that shows `TextLog` entries in a table and scrolls with the active time.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

mod view_class;
mod visualizer_system;

pub use view_class::TextView;
