// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configuration for the legend of a plot.
#[rerun::rerun_type]
#[python(aliases = "blueprint_components.Corner2D")]
#[rerun(scope = "blueprint")]
#[rust(derive(Default))]
#[rerun(state = "unstable")]
pub struct PlotLegend {
    /// To what corner the legend is aligned.
    ///
    /// Defaults to the right bottom corner.
    #[rerun(optional)]
    pub corner: Option<rerun::blueprint::components::Corner2D>,

    /// Whether the legend is shown at all.
    ///
    /// True by default.
    #[rerun(optional)]
    pub visible: Option<rerun::components::Visible>,
}
