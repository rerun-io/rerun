// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configuration for the time (X) axis of a plot.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default))]
#[rerun(state = "unstable")]
pub struct TimeAxis {
    /// How should the horizontal/X/time axis be linked across multiple plots?
    ///
    /// Linking with global will ignore `view_range`.
    #[rerun(optional)]
    pub link: Option<rerun::blueprint::components::LinkAxis>,

    /// The view range of the horizontal/X/time axis.
    #[rerun(optional)]
    pub view_range: Option<rerun::blueprint::components::TimeRange>,

    /// If enabled, the X axis range will remain locked to the specified range when zooming.
    #[rerun(optional)]
    pub zoom_lock: Option<rerun::blueprint::components::LockRangeDuringZoom>,
}
