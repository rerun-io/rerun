// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Formatting options for a raw data view.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct RawDataViewFormat {
    /// Show the bar carrying the time cursor and the step-to-next-event buttons.
    ///
    /// Defaults to true.
    #[rerun(optional)]
    pub show_time_navigation: Option<rerun::components::Visible>,

    /// How densely the rows are packed.
    ///
    /// Defaults to spacious.
    #[rerun(optional)]
    pub style: Option<rerun::blueprint::components::ViewDensity>,
}
