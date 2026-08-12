// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configuration for the background of a spatial view.
#[rerun::rerun_type]
#[python(aliases = "datatypes.Rgba32Like | blueprint_components.BackgroundKindLike")]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct Background {
    /// The type of the background.
    #[rerun(required)]
    pub kind: rerun::blueprint::components::BackgroundKind,

    /// Color used for the solid background type.
    // TODO(andreas): Can't link to [components.BackgroundKind.SolidColor] since blueprint components aren't part of the doc page yet.
    #[rerun(optional)]
    pub color: Option<rerun::components::Color>,
}

/// Configuration of a background in a graph view.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct GraphBackground {
    /// Color used for the background.
    #[rerun(optional)]
    pub color: Option<rerun::components::Color>,
}

/// Configuration of a background in a plot view.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct PlotBackground {
    /// Color used for the background.
    #[rerun(optional)]
    pub color: Option<rerun::components::Color>,

    /// Should the grid be drawn?
    #[rerun(optional)]
    pub show_grid: Option<rerun::blueprint::components::Enabled>,
}
