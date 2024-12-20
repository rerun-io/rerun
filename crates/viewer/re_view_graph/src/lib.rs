//! Rerun Graph View.
//!
//! A View that shows a graph (node-link diagram).

mod graph;
mod layout;
mod properties;
mod ui;
mod view;
mod visualizers;

#[cfg(test)]
pub use ui::GraphViewState;
pub use view::GraphView;

mod tmp; // TODO: do not merge
