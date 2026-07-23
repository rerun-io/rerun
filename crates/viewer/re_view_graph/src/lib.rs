//! Rerun Graph View.
//!
//! A View that shows a graph (node-link diagram).

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

mod graph;
mod layout;
mod ui;
mod view;
mod visualizers;

pub use ui::GraphViewState;
pub use view::GraphView;
