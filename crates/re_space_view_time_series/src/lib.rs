//! Rerun time series Space View
//!
//! A Space View that shows plots over Rerun timelines.

mod aggregation;
mod legacy_visualizer_system;
mod line_visualizer_system;
mod overrides;
mod point_visualizer_system;
mod space_view_class;
mod util;

use re_log_types::EntityPath;
use re_types::components::MarkerShape;
use re_viewer_context::external::re_entity_db::TimeSeriesAggregator;
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

// ---

#[derive(Clone, Debug)]
pub struct PlotPointAttrs {
    pub label: Option<String>,
    pub color: egui::Color32,
    pub stroke_width: f32,
    pub kind: PlotSeriesKind,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct ScatterAttrs {
    pub marker: MarkerShape,
}

impl PartialEq for PlotPointAttrs {
    fn eq(&self, rhs: &Self) -> bool {
        let Self {
            label,
            color,
            stroke_width: radius,
            kind,
        } = self;
        label.eq(&rhs.label)
            && color.eq(&rhs.color)
            && radius.total_cmp(&rhs.stroke_width).is_eq()
            && kind.eq(&rhs.kind)
    }
}

impl Eq for PlotPointAttrs {}

#[derive(Clone, Debug)]
struct PlotPoint {
    time: i64,
    value: f64,
    attrs: PlotPointAttrs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotSeriesKind {
    Continuous,
    Scatter(ScatterAttrs),
    Clear,
}

#[derive(Clone, Debug)]
pub struct PlotSeries {
    pub label: String,
    pub color: egui::Color32,
    pub width: f32,
    pub kind: PlotSeriesKind,
    pub points: Vec<(i64, f64)>,
    pub entity_path: EntityPath,

    /// Earliest time an entity was recorded at on the current timeline.
    pub min_time: i64,

    /// What kind of aggregation was used to compute the graph?
    pub aggregator: TimeSeriesAggregator,

    /// `1.0` for raw data.
    ///
    /// How many raw data points were aggregated into a single step of the graph?
    /// This is an average.
    pub aggregation_factor: f64,
}
