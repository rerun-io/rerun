//! Rerun Time Panel
//!
//! This crate provides a panel that shows all entities in the store and allows control of time and
//! timelines, as well as all necessary ui elements that make it up.

mod data_density_graph;
mod paint_ticks;
mod recursive_chunks_per_timeline_subscriber;
mod time_axis;
mod time_control_ui;
mod time_panel;
mod time_ranges_ui;
mod time_selection_ui;

#[cfg(feature = "testing")]
pub mod streams_tree_data;

#[cfg(not(feature = "testing"))]
mod streams_tree_data;

pub use time_panel::TimePanel;

#[cfg(feature = "testing")]
pub use time_panel::TimePanelSource;

#[doc(hidden)]
pub mod __bench {
    pub use crate::data_density_graph::*;
    pub use crate::time_panel::TimePanelItem;
    pub use crate::time_ranges_ui::TimeRangesUi;
}
