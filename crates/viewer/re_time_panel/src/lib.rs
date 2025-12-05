//! Rerun Time Panel
//!
//! This crate provides a panel that shows all entities in the store and allows control of time and
//! timelines, as well as all necessary ui elements that make it up.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

mod data_density_graph;
mod paint_ticks;
mod recursive_chunks_per_timeline_subscriber;
mod streams_tree_data;
mod time_axis;
mod time_control_ui;
mod time_panel;
mod time_ranges_ui;
mod time_selection_ui;

pub use time_panel::TimePanel;
#[cfg(feature = "testing")]
pub use {streams_tree_data::StreamsTreeData, time_panel::TimePanelSource};

#[doc(hidden)]
pub mod __bench {
    pub use crate::data_density_graph::*;
    pub use crate::time_panel::TimePanelItem;
    pub use crate::time_ranges_ui::TimeRangesUi;
}

/// Indicate moving the time cursor.
const MOVE_TIME_CURSOR_ICON: egui::CursorIcon = egui::CursorIcon::ResizeColumn;

/// Indicate creating a new time loop selection.
const CREATE_TIME_LOOP_CURSOR_ICON: egui::CursorIcon = egui::CursorIcon::Default;
// const CREATE_TIME_LOOP_CURSOR_ICON: egui::CursorIcon = egui::CursorIcon::ResizeHorizontal;   // TODO(rust-windowing/winit#4390)
