//! Rerun time series Space View
//!
//! A Space View that shows plots over Rerun timelines.

mod space_view_class;
mod visualizer_system;

pub use space_view_class::TimeSeriesSpaceView;

/// Computes a deterministic, globally unique ID for the plot based on the ID of the space view
/// itself.
///
/// Use it to access the plot's state from anywhere, e.g.:
/// ```ignore
/// let plot_mem = egui_plot::PlotMemory::load(egui_ctx, crate::plot_id(query.space_view_id));
/// ```
#[inline]
pub(crate) fn plot_id(space_view_id: re_viewer_context::SpaceViewId) -> egui::Id {
    egui::Id::new(("plot", space_view_id))
}
