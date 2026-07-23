//! Rerun map visualization View.
//!
//! A View that shows geographic objects on a map.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

mod map_overlays;
mod map_view;
mod visualizers;

pub use map_view::MapView;
