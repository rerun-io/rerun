// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configuration for plot interaction behavior.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default))]
#[rerun(state = "unstable")]
pub struct PlotInteraction {
    /// How the tooltip behaves when hovering over the plot.
    #[rerun(optional)]
    pub tooltip_mode: Option<rerun::blueprint::components::TooltipMode>,

    /// When data point markers are displayed on line series.
    #[rerun(optional)]
    pub points_display: Option<rerun::blueprint::components::PointsDisplay>,
}
