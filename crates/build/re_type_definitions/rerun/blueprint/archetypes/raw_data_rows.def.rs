// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configuration of the rows shown by a raw data view.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct RawDataRows {
    /// The components to display.
    ///
    /// If specified, components not listed are hidden.
    /// If not specified, all components of the entity are shown.
    #[rerun(optional)]
    pub data_rows: Option<Vec<rerun::blueprint::components::ComponentIdentifier>>,
}
