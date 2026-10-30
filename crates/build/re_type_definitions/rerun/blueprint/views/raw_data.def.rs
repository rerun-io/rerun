// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A view that shows the raw component data of a single entity.
///
/// \example views/raw_data title="Use a blueprint to show a RawDataView."
#[rerun::rerun_type]
#[docs(archetype_agnostic)]
#[rerun(view_identifier = "RawData")]
#[rerun(state = "unstable")]
pub struct RawDataView {
    /// The rows to display.
    pub rows: rerun::blueprint::archetypes::RawDataRows,

    /// Formatting options for the raw data view.
    pub format_options: rerun::blueprint::archetypes::RawDataViewFormat,
}
