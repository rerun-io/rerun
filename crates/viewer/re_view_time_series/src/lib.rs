//! Rerun time series View
//!
//! A View that shows plots over Rerun timelines.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

mod aggregation;
mod fallbacks;
mod line_visualizer_system;
mod naming;
mod point_visualizer_system;
mod series_query;
mod util;
mod view_class;

use re_log_types::AbsoluteTimeRange;
use re_sdk_types::{
    blueprint::components::VisualizerInstructionId,
    components::{AggregationPolicy, MarkerShape},
};
use re_viewer_context::external::re_entity_db::InstancePath;
use re_viewport_blueprint::ViewPropertyQueryError;
pub use view_class::TimeSeriesView;

/// Computes a deterministic, globally unique ID for the plot based on the ID of the view
/// itself.
///
/// Use it to access the plot's state from anywhere, e.g.:
/// ```ignore
/// let plot_mem = egui_plot::PlotMemory::load(egui_ctx, crate::plot_id(query.view_id));
/// ```
#[inline]
pub(crate) fn plot_id(view_id: re_viewer_context::ViewId) -> egui::Id {
    egui::Id::new(("plot", view_id))
}

// ---

#[derive(Clone, Debug)]
pub struct PlotPointAttrs {
    pub color: egui::Color32,

    /// Radius of markers, or stroke radius for lines.
    pub radius_ui: f32,

    pub kind: PlotSeriesKind,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct ScatterAttrs {
    pub marker: MarkerShape,
}

impl PartialEq for PlotPointAttrs {
    fn eq(&self, rhs: &Self) -> bool {
        let Self {
            color,
            radius_ui,
            kind,
        } = self;
        color.eq(&rhs.color) && radius_ui.total_cmp(&rhs.radius_ui).is_eq() && kind.eq(&rhs.kind)
    }
}

impl Eq for PlotPointAttrs {}

#[derive(Clone, Debug, PartialEq)]
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
    pub instance_path: InstancePath,

    /// The views' visible time range.
    pub visible_time_range: AbsoluteTimeRange,

    /// Id of the visualizer instruction that is responsible for this series.
    pub visualizer_instruction_id: VisualizerInstructionId,

    /// Whether the individual series is visible.
    ///
    /// If this is false, [`PlotSeries::points`] is allowed to be empty.
    pub visible: bool,

    /// Label of the series.
    pub label: String,

    pub color: egui::Color32,

    /// Radius of markers, or stroke radius for lines.
    pub radius_ui: f32,

    pub kind: PlotSeriesKind,
    pub points: Vec<(i64, f64)>,

    /// Earliest time an entity was recorded at on the current timeline.
    pub min_time: i64,

    /// What kind of aggregation was used to compute the graph?
    pub aggregator: AggregationPolicy,

    /// `1.0` for raw data.
    ///
    /// How many raw data points were aggregated into a single step of the graph?
    /// This is an average.
    pub aggregation_factor: f64,
}

impl PlotSeries {
    /// Returns a unique id for a given plot series.
    ///
    /// NOTE: A single visualizer instruction can be responsible for multiple series,
    /// so we use the instance path number as an additional differentiator.
    pub fn id(&self) -> egui::Id {
        egui::Id::new((&self.visualizer_instruction_id, self.instance_path.instance))
    }
}
