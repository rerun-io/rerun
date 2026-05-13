//! Rerun state timeline View.
//!
//! A View that shows state transitions as horizontal lanes over time.

mod data;
mod view_class;
mod visualizer;
mod visualizer_ui;

pub use data::{StateLane, StateLanePhase, StateLanesData};
pub use view_class::StateTimelineView;
pub use visualizer::StateVisualizer;
