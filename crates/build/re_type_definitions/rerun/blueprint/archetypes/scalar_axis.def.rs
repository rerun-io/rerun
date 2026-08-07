// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configuration for the scalar (Y) axis of a plot.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default))]
#[rerun(state = "unstable")]
pub struct ScalarAxis {
    /// The range of the axis.
    ///
    /// If unset, the range well be automatically determined based on the queried data.
    #[rerun(optional)]
    pub range: Option<rerun::components::Range1D>,

    /// If enabled, the Y axis range will remain locked to the specified range when zooming.
    #[rerun(optional)]
    pub zoom_lock: Option<rerun::blueprint::components::LockRangeDuringZoom>,
}
