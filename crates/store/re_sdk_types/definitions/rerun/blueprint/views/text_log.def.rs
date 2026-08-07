// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A view of a text log, for use with [archetypes.TextLog].
///
/// \example views/text_log title="Use a blueprint to show a TextLogView." image="https://static.rerun.io/text_log/457ab91ec42a481bacae4146c0fc01eee397bb86/1200w.png"
#[rerun::rerun_type]
#[rerun(view_identifier = "TextLog")]
#[rerun(state = "unstable")]
pub struct TextLogView {
    /// The columns to display in the view.
    pub columns: rerun::blueprint::archetypes::TextLogColumns,

    /// Filter for rows to display in the view.
    pub rows: rerun::blueprint::archetypes::TextLogRows,

    /// Formatting options for the text log view.
    pub format_options: rerun::blueprint::archetypes::TextLogFormat,
}
