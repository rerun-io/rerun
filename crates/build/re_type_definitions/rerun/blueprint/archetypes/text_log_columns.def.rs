// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configuration of the text log columns.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct TextLogColumns {
    /// What timeline columns to show.
    ///
    /// Defaults to displaying only the active timeline.
    #[rerun(optional)]
    pub timeline_columns: Option<Vec<rerun::blueprint::components::TimelineColumn>>,

    /// All columns to be displayed.
    ///
    /// Defaults to showing all text log column kinds in the order of the enum.
    #[rerun(optional)]
    pub text_log_columns: Option<Vec<rerun::blueprint::components::TextLogColumn>>,
}
