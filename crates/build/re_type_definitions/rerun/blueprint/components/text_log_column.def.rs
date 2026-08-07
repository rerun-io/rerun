// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A text log column
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(PartialEq, Eq, Default, Hash))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct TextLogColumn {
    /// The text log column.
    pub text_log_column: rerun::blueprint::datatypes::TextLogColumn,
}
