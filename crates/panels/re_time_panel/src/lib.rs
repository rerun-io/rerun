//! Rerun Time Panel
//!
//! This crate provides a panel that shows all entities in the store and allows control of time and
//! timelines, as well as all necessary ui elements that make it up.

mod data_density_graph;
mod recursive_chunks_per_timeline_subscriber;
mod streams_tree_data;
mod time_axis;
mod time_control_ui;
mod time_panel;
mod time_selection_ui;

pub use time_panel::TimePanel;
#[cfg(feature = "testing")]
pub use {streams_tree_data::StreamsTreeData, time_panel::TimePanelSource};

#[doc(hidden)]
pub mod __bench {
    pub use crate::data_density_graph::*;
    pub use crate::time_panel::TimePanelItem;
    pub use re_time_ruler::TimeRangesUi;
}

/// Indicate creating a new time loop selection.
// Winit 0.30.13 still uses the deprecated macOS resize cursor mapping.
// ResizeHorizontal requires the new mapping and visual verification on macOS.
// See https://github.com/rust-windowing/winit/issues/4390;
// upstream fix: https://github.com/rust-windowing/winit/pull/4422.
const CREATE_TIME_LOOP_CURSOR_ICON: egui::CursorIcon = egui::CursorIcon::Default;
